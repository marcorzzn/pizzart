//! Il documento: dimensioni, albero dei nodi, storia delle modifiche.
//!
//! L'albero (`root`) è composto da [`Node`]: un layer con pixel, o un gruppo con
//! figli. I gruppi applicano sempre blend/opacità/maschera al proprio composito.

use crate::compositor;
use crate::error::LayerError;
use crate::history::{
    Command, CreateGroup, Duplicate, FlattenGroup, History, MergeDown, Reorder, Ungroup,
    DEFAULT_HISTORY_LIMIT,
};
use crate::layer::{Layer, LayerId};
use crate::node::{self, Group, InsertWhere, Node};

/// Un'immagine modificabile: un albero di layer e gruppi, più la storia.
pub struct Document {
    pub width: u32,
    pub height: u32,
    /// Dal fondo alla cima: `root[0]` è il livello più basso.
    pub root: Vec<Node>,
    pub history: History,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            root: Vec::new(),
            history: History::new(DEFAULT_HISTORY_LIMIT),
        }
    }

    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    // -----------------------------------------------------------------
    // Lettura dell'albero
    // -----------------------------------------------------------------

    /// Tutti i layer, appiattendo i gruppi, dal fondo alla cima.
    ///
    /// È la vista "piatta" usata da chi disegna o esporta: l'ordine è quello di
    /// disegno, e un gruppo viene attraversato nell'ordine dei suoi figli.
    pub fn layers(&self) -> Vec<&Layer> {
        let mut out = Vec::new();
        node::flatten(&self.root, &mut out);
        out
    }

    pub fn node(&self, id: LayerId) -> Option<&Node> {
        node::find(&self.root, id)
    }

    pub fn node_mut(&mut self, id: LayerId) -> Option<&mut Node> {
        node::find_mut(&mut self.root, id)
    }

    pub fn group(&self, id: LayerId) -> Option<&Group> {
        self.node(id)?.as_group()
    }

    pub fn group_mut(&mut self, id: LayerId) -> Option<&mut Group> {
        self.node_mut(id)?.as_group_mut()
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.node(id)?.as_layer()
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.node_mut(id)?.as_layer_mut()
    }

    /// Il contenitore che ospita il nodo: `Some(None)` per la radice.
    pub fn parent_of(&self, id: LayerId) -> Option<Option<LayerId>> {
        node::parent_of(&self.root, id)
    }

    /// I figli del contenitore (radice o gruppo).
    pub fn children_of(&self, parent: Option<LayerId>) -> Option<&[Node]> {
        match parent {
            None => Some(&self.root),
            Some(id) => Some(&self.group(id)?.children),
        }
    }

    /// Posizione fra i fratelli, oppure `None`.
    pub fn index_of(&self, id: LayerId) -> Option<usize> {
        let parent = self.parent_of(id)?;
        node::position_of(self.children_of(parent)?, id)
    }

    pub fn layer_count(&self) -> usize {
        node::count_layers(&self.root)
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_empty()
    }

    pub fn visible_layer_count(&self) -> usize {
        self.layers().iter().filter(|l| l.visible).count()
    }

    // -----------------------------------------------------------------
    // Modifica della struttura (senza storia: la storia è nei comandi)
    // -----------------------------------------------------------------

    /// Aggiunge un layer in cima alla radice.
    pub fn add_layer(&mut self, layer: Layer) -> LayerId {
        let id = layer.id;
        self.root.push(Node::Layer(layer));
        id
    }

    /// Aggiunge un nodo (layer o gruppo) in cima alla radice.
    pub fn add_node(&mut self, node: Node) -> LayerId {
        let id = node.id();
        self.root.push(node);
        id
    }

    /// Inserisce un layer alla posizione data nella radice, con clamping.
    pub fn insert_layer(&mut self, index: usize, layer: Layer) {
        let index = index.min(self.root.len());
        self.root.insert(index, Node::Layer(layer));
    }

    /// Rimuove il nodo alla posizione data della radice.
    pub fn remove_at(&mut self, index: usize) -> Option<Node> {
        if index < self.root.len() {
            Some(self.root.remove(index))
        } else {
            None
        }
    }

    /// Rimuove un nodo (anche annidato) e lo restituisce.
    pub fn remove_node(&mut self, id: LayerId) -> Option<Node> {
        node::remove(&mut self.root, id)
    }

    /// Inserisce un nodo in un contenitore, con clamping.
    pub fn insert_node(&mut self, parent: Option<LayerId>, index: usize, node: Node) -> bool {
        node::insert_into(&mut self.root, parent, index, node)
    }

    // -----------------------------------------------------------------
    // Operazioni di alto livello: validano, applicano, registrano
    // -----------------------------------------------------------------

    /// Duplica un nodo (con maschere clonate) subito sopra l'originale.
    pub fn duplicate(&mut self, id: LayerId) -> Result<LayerId, LayerError> {
        let parent = self.parent_of(id).ok_or(LayerError::NotFound)?;
        let index = self.index_of(id).ok_or(LayerError::NotFound)?;
        let copia = self.node(id).ok_or(LayerError::NotFound)?.deep_clone();
        let nuovo = copia.id();
        let mut cmd = Duplicate::new(id, nuovo, parent, index + 1);
        cmd.apply(self);
        self.history.push(Box::new(cmd));
        Ok(nuovo)
    }

    /// Fonde il layer sopra in quello sotto.
    pub fn merge_down(&mut self, top: LayerId) -> Result<(), LayerError> {
        let parent = self.parent_of(top).ok_or(LayerError::NotFound)?;
        let index = self.index_of(top).ok_or(LayerError::NotFound)?;
        if index == 0 {
            return Err(LayerError::CannotMergeDown);
        }
        // il layer sotto deve essere un layer, non un gruppo
        let sotto_id = match self.children_of(parent).and_then(|c| c.get(index - 1)) {
            Some(Node::Layer(l)) => l.id,
            _ => return Err(LayerError::CannotMergeDown),
        };
        let mut cmd = MergeDown::new(sotto_id, top, parent);
        cmd.apply(self);
        self.history.push(Box::new(cmd));
        Ok(())
    }

    /// Sostituisce un gruppo con un layer che è il suo composito.
    pub fn flatten_group(&mut self, id: LayerId) -> Result<LayerId, LayerError> {
        let parent = self.parent_of(id).ok_or(LayerError::NotFound)?;
        let index = self.index_of(id).ok_or(LayerError::NotFound)?;
        if self.group(id).is_none() {
            return Err(LayerError::NotAGroup);
        }
        let mut cmd = FlattenGroup::new(id, parent, index, self.width, self.height);
        let nuovo = cmd.do_apply(self)?;
        self.history.push(Box::new(cmd));
        Ok(nuovo)
    }

    /// Raggruppa i nodi indicati (che devono essere fratelli).
    pub fn create_group(&mut self, ids: &[LayerId]) -> Result<LayerId, LayerError> {
        if ids.is_empty() {
            return Err(LayerError::EmptySelection);
        }
        let parent = self.parent_of(ids[0]).ok_or(LayerError::NotFound)?;
        for id in ids {
            if self.parent_of(*id) != Some(parent) {
                return Err(LayerError::NotSiblings);
            }
        }
        let mut cmd = CreateGroup::new(ids.to_vec(), parent, "Gruppo");
        let nuovo = cmd.do_apply(self)?;
        self.history.push(Box::new(cmd));
        Ok(nuovo)
    }

    /// Scioglie un gruppo: i figli prendono il suo posto.
    pub fn ungroup(&mut self, id: LayerId) -> Result<(), LayerError> {
        let parent = self.parent_of(id).ok_or(LayerError::NotFound)?;
        let index = self.index_of(id).ok_or(LayerError::NotFound)?;
        if self.group(id).is_none() {
            return Err(LayerError::NotAGroup);
        }
        let mut cmd = Ungroup::new(id, parent, index);
        cmd.do_apply(self)?;
        self.history.push(Box::new(cmd));
        Ok(())
    }

    /// Sposta un nodo rispetto a un altro.
    pub fn reorder(
        &mut self,
        id: LayerId,
        target: LayerId,
        insert: InsertWhere,
    ) -> Result<(), LayerError> {
        if id == target {
            return Ok(());
        }
        let parent = self.parent_of(id).ok_or(LayerError::NotFound)?;
        let target_parent = self.parent_of(target).ok_or(LayerError::NotFound)?;
        match insert {
            InsertWhere::Before | InsertWhere::After => {
                if parent != target_parent {
                    return Err(LayerError::InvalidTarget);
                }
            }
            InsertWhere::Inside => {
                if self.group(target).is_none() {
                    return Err(LayerError::InvalidTarget);
                }
                // un gruppo non può finire dentro sé stesso
                if self.group(id).is_some() && self.is_ancestor(id, target) {
                    return Err(LayerError::InvalidTarget);
                }
            }
        }
        let mut cmd = Reorder::new(id, target, insert);
        cmd.apply(self);
        self.history.push(Box::new(cmd));
        Ok(())
    }

    /// `true` se `ancestor` contiene `id` (a qualsiasi profondità).
    pub fn is_ancestor(&self, ancestor: LayerId, id: LayerId) -> bool {
        match self.group(ancestor) {
            Some(g) => node::find(&g.children, id).is_some(),
            None => false,
        }
    }

    // -----------------------------------------------------------------
    // Storia e compositing
    // -----------------------------------------------------------------

    /// Applica un comando e lo registra nella storia.
    pub fn execute(&mut self, mut cmd: Box<dyn Command>) {
        cmd.apply(self);
        self.history.push(cmd);
    }

    pub fn undo(&mut self) -> bool {
        // La storia vive dentro il documento, ma i comandi hanno bisogno di
        // `&mut Document`: la si sposta fuori per la durata della chiamata.
        let mut history = std::mem::take(&mut self.history);
        let done = history.undo(self);
        self.history = history;
        done
    }

    pub fn redo(&mut self) -> bool {
        let mut history = std::mem::take(&mut self.history);
        let done = history.redo(self);
        self.history = history;
        done
    }

    /// Compone l'albero in RGBA8 (riferimento CPU, nessuna GPU).
    pub fn composite(&self) -> Vec<u8> {
        compositor::composite(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::AddLayer;
    use crate::transform::Transform;

    #[test]
    fn un_documento_nuovo_e_vuoto_con_storia_vergine() {
        let doc = Document::new(100, 50);
        assert_eq!(doc.width, 100);
        assert_eq!(doc.height, 50);
        assert_eq!(doc.pixel_count(), 5000);
        assert!(doc.is_empty());
        assert!(!doc.history.can_undo());
        assert_eq!(doc.history.limit(), DEFAULT_HISTORY_LIMIT);
    }

    #[test]
    fn execute_applica_e_registra() {
        let mut doc = Document::new(4, 4);
        doc.execute(Box::new(AddLayer::new(Layer::filled(
            "l",
            4,
            4,
            [9, 9, 9, 255],
        ))));
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.history.len(), 1);
        assert_eq!(doc.visible_layer_count(), 1);
    }

    #[test]
    fn il_composite_riflette_le_modifiche_annullate() {
        let mut doc = Document::new(2, 2);
        let white = Layer::filled("bianco", 2, 2, [255, 255, 255, 255]);
        doc.execute(Box::new(AddLayer::new(white)));
        assert_eq!(&doc.composite()[0..4], &[255, 255, 255, 255]);

        doc.undo();
        assert_eq!(
            &doc.composite()[0..4],
            &[0, 0, 0, 0],
            "documento di nuovo vuoto"
        );

        doc.redo();
        assert_eq!(&doc.composite()[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn insert_e_remove_rispettano_i_bordi() {
        let mut doc = Document::new(2, 2);
        doc.insert_layer(99, Layer::filled("a", 2, 2, [0, 0, 0, 255]));
        assert_eq!(doc.layer_count(), 1);
        assert!(doc.remove_at(5).is_none());
        assert!(doc.remove_at(0).is_some());
        assert!(doc.is_empty());
    }

    #[test]
    fn la_trasformazione_di_un_layer_si_vede_nel_risultato() {
        let mut doc = Document::new(4, 4);
        let mut l = Layer::filled("punto", 1, 1, [255, 0, 0, 255]);
        l.transform = Transform::at(2.0, 2.0);
        doc.execute(Box::new(AddLayer::new(l)));
        let out = doc.composite();
        let i = (2 * 4 + 2) * 4;
        assert_eq!(&out[i..i + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn layers_appiattisce_i_gruppi_nell_ordine_di_disegno() {
        let mut doc = Document::new(2, 2);
        let a = doc.add_layer(Layer::filled("a", 2, 2, [1, 0, 0, 255]));
        let mut g = Group::new("g");
        g.add(Node::Layer(Layer::filled("b", 2, 2, [2, 0, 0, 255])));
        g.add(Node::Layer(Layer::filled("c", 2, 2, [3, 0, 0, 255])));
        doc.add_node(Node::Group(g));
        let nomi: Vec<&str> = doc.layers().iter().map(|l| l.name.as_str()).collect();
        assert_eq!(nomi, vec!["a", "b", "c"]);
        assert_eq!(doc.layer(a).unwrap().name, "a");
        assert_eq!(doc.layer_count(), 3);
    }
}
