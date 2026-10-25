//! Storia delle modifiche: comandi reversibili.
//!
//! Ogni modifica passa da un `Command` che sa applicarsi e disfarsi. La storia
//! ha due pile (fatte / disfatte) e un limite: superato il limite si perde il
//! comando più vecchio, non quello più recente, così l'undo resta immediato
//! anche con centinaia di passi.

use std::sync::Arc;

use crate::blend::BlendMode;
use crate::compositor;
use crate::document::Document;
use crate::error::LayerError;
use crate::layer::{Layer, LayerId};
use crate::mask::Mask;
use crate::node::{Group, InsertWhere, Node};
use crate::transform::Transform;

/// Una modifica reversibile sul documento.
pub trait Command {
    fn apply(&mut self, doc: &mut Document);
    fn undo(&mut self, doc: &mut Document);
    /// Nome mostrato nello storico ("Opacità layer", "Aggiungi livello", ...).
    fn label(&self) -> &str;
}

/// Numero di passi di undo tenuti in memoria per impostazione predefinita.
pub const DEFAULT_HISTORY_LIMIT: usize = 1000;

pub struct History {
    done: Vec<Box<dyn Command>>,
    undone: Vec<Box<dyn Command>>,
    limit: usize,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Registra un comando già applicato. Scarta la pila dei redo,
    /// perché la storia lineare non ammette diramazioni.
    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.done.push(cmd);
        self.undone.clear();
        while self.done.len() > self.limit {
            self.done.remove(0);
        }
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        match self.done.pop() {
            Some(mut cmd) => {
                cmd.undo(doc);
                self.undone.push(cmd);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        match self.undone.pop() {
            Some(mut cmd) => {
                cmd.apply(doc);
                self.done.push(cmd);
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn len(&self) -> usize {
        self.done.len()
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Etichette della storia, dalla più vecchia alla più recente.
    pub fn labels(&self) -> Vec<&str> {
        self.done.iter().map(|c| c.label()).collect()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_LIMIT)
    }
}

/// Aggiunge un layer (eventualmente in una posizione precisa).
pub struct AddLayer {
    layer: Option<Layer>,
    index: Option<usize>,
    /// L'identificativo del nodo inserito: è ciò che l'undo deve togliere,
    /// senza doverlo ricalcolare (i gruppi rendono gli indici ambigui).
    added: Option<LayerId>,
}

impl AddLayer {
    pub fn new(layer: Layer) -> Self {
        Self {
            layer: Some(layer),
            index: None,
            added: None,
        }
    }

    pub fn at(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }
}

impl Command for AddLayer {
    // `layer` resta di proprietà del comando: serve anche al redo, quindi
    // ogni apply inserisce una copia.
    fn apply(&mut self, doc: &mut Document) {
        if let Some(layer) = &self.layer {
            let id = layer.id;
            match self.index {
                Some(i) => doc.insert_layer(i.min(doc.layer_count()), layer.clone()),
                None => {
                    doc.add_layer(layer.clone());
                }
            }
            self.added = Some(id);
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(id) = self.added.take() {
            doc.remove_node(id);
        }
    }

    fn label(&self) -> &str {
        "Aggiungi livello"
    }
}

/// Rimuove un layer, ricordandone posizione e contenuto.
pub struct RemoveLayer {
    id: LayerId,
    removed: Option<(usize, Layer)>,
}

impl RemoveLayer {
    pub fn new(id: LayerId) -> Self {
        Self { id, removed: None }
    }
}

impl Command for RemoveLayer {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(index) = doc.index_of(self.id) {
            self.removed = doc
                .remove_at(index)
                .and_then(|node| match node {
                    Node::Layer(l) => Some(l),
                    Node::Group(_) => None,
                })
                .map(|layer| (index, layer));
        } else if let Some((index, layer)) = &self.removed {
            // redo dopo un undo: il layer è ancora nostro, si reinserisce e
            // si rimuove di nuovo per lasciare lo stato coerente
            let (index, layer) = (*index, layer.clone());
            doc.insert_layer(index, layer.clone());
            self.removed = doc
                .remove_at(index)
                .and_then(|node| match node {
                    Node::Layer(l) => Some(l),
                    Node::Group(_) => None,
                })
                .map(|l| (index, l));
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some((index, layer)) = self.removed.take() {
            doc.insert_layer(index.min(doc.layer_count()), layer);
        }
    }

    fn label(&self) -> &str {
        "Rimuovi livello"
    }
}

/// Cambia l'opacità di un layer.
pub struct SetOpacity {
    id: LayerId,
    new: f32,
    old: Option<f32>,
}

impl SetOpacity {
    pub fn new(id: LayerId, value: f32) -> Self {
        Self {
            id,
            new: value.clamp(0.0, 1.0),
            old: None,
        }
    }
}

impl Command for SetOpacity {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.layer_mut(self.id) {
            self.old = Some(layer.opacity);
            layer.opacity = self.new;
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let (Some(old), Some(layer)) = (self.old, doc.layer_mut(self.id)) {
            layer.opacity = old;
        }
    }

    fn label(&self) -> &str {
        "Opacità livello"
    }
}

/// Cambia posizione, scala e rotazione di un layer.
pub struct SetTransform {
    id: LayerId,
    new: Transform,
    old: Option<Transform>,
}

impl SetTransform {
    pub fn new(id: LayerId, value: Transform) -> Self {
        Self {
            id,
            new: value,
            old: None,
        }
    }
}

impl Command for SetTransform {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.layer_mut(self.id) {
            self.old = Some(layer.transform);
            layer.transform = self.new;
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let (Some(old), Some(layer)) = (self.old, doc.layer_mut(self.id)) {
            layer.transform = old;
        }
    }

    fn label(&self) -> &str {
        "Trasforma livello"
    }
}

/// Cambia il nome di un layer.
pub struct RenameLayer {
    id: LayerId,
    new: String,
    old: Option<String>,
}

impl RenameLayer {
    pub fn new(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            new: name.into(),
            old: None,
        }
    }
}

impl Command for RenameLayer {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.layer_mut(self.id) {
            self.old = Some(std::mem::replace(&mut layer.name, self.new.clone()));
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let (Some(old), Some(layer)) = (self.old.take(), doc.layer_mut(self.id)) {
            layer.name = old;
        }
    }

    fn label(&self) -> &str {
        "Rinomina livello"
    }
}

/// Cambia il blend mode di un layer.
pub struct SetBlendMode {
    id: LayerId,
    new: BlendMode,
    old: Option<BlendMode>,
}

impl SetBlendMode {
    pub fn new(id: LayerId, mode: BlendMode) -> Self {
        Self {
            id,
            new: mode,
            old: None,
        }
    }
}

impl Command for SetBlendMode {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.layer_mut(self.id) {
            self.old = Some(layer.blend);
            layer.blend = self.new;
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let (Some(old), Some(layer)) = (self.old, doc.layer_mut(self.id)) {
            layer.blend = old;
        }
    }

    fn label(&self) -> &str {
        "Modo di fusione"
    }
}

/// Mostra o nasconde un layer.
pub struct SetVisible {
    id: LayerId,
    new: bool,
    old: Option<bool>,
}

impl SetVisible {
    pub fn new(id: LayerId, visible: bool) -> Self {
        Self {
            id,
            new: visible,
            old: None,
        }
    }
}

impl Command for SetVisible {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(node) = doc.node_mut(self.id) {
            self.old = Some(node.as_layer().map(|l| l.visible).unwrap_or(true));
            match node {
                Node::Layer(l) => l.visible = self.new,
                Node::Group(g) => g.visible = self.new,
            }
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let (Some(old), Some(node)) = (self.old, doc.node_mut(self.id)) {
            match node {
                Node::Layer(l) => l.visible = old,
                Node::Group(g) => g.visible = old,
            }
        }
    }

    fn label(&self) -> &str {
        "Visibilita"
    }
}
/// Assegna (o toglie) la maschera di un layer.
pub struct SetLayerMask {
    id: LayerId,
    new: Option<Arc<Mask>>,
    old: Option<Option<Arc<Mask>>>,
}

impl SetLayerMask {
    pub fn new(id: LayerId, mask: Option<Arc<Mask>>) -> Self {
        Self {
            id,
            new: mask,
            old: None,
        }
    }
}

impl Command for SetLayerMask {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.layer_mut(self.id) {
            self.old = Some(layer.mask.take());
            layer.mask = self.new.clone();
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let (Some(old), Some(layer)) = (self.old.take(), doc.layer_mut(self.id)) {
            layer.mask = old;
        }
    }

    fn label(&self) -> &str {
        "Maschera livello"
    }
}

/// Inverte una maschera esistente.
///
/// La maschera è condivisa (`Arc`): per invertirla si crea una copia con il
/// flag cambiato, così un altro documento che la usa non cambia aspetto.
pub struct ToggleMaskInverted {
    id: LayerId,
    old: Option<Option<Arc<Mask>>>,
}

impl ToggleMaskInverted {
    pub fn new(id: LayerId) -> Self {
        Self { id, old: None }
    }
}

impl Command for ToggleMaskInverted {
    fn apply(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.layer_mut(self.id) {
            if let Some(attuale) = &layer.mask {
                let invertita = Arc::new(attuale.as_ref().clone().with_inverted(!attuale.inverted));
                self.old = Some(layer.mask.take());
                layer.mask = Some(invertita);
            }
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let (Some(old), Some(layer)) = (self.old.take(), doc.layer_mut(self.id)) {
            layer.mask = old;
        }
    }

    fn label(&self) -> &str {
        "Inverti maschera"
    }
}

/// Duplica un nodo, con maschere clonate, subito sopra l'originale.
pub struct Duplicate {
    source: LayerId,
    new_id: LayerId,
    parent: Option<LayerId>,
    index: usize,
    created: bool,
}

impl Duplicate {
    pub fn new(source: LayerId, new_id: LayerId, parent: Option<LayerId>, index: usize) -> Self {
        Self {
            source,
            new_id,
            parent,
            index,
            created: false,
        }
    }
}

impl Command for Duplicate {
    fn apply(&mut self, doc: &mut Document) {
        let Some(originale) = doc.node(self.source) else {
            return;
        };
        let mut copia = originale.deep_clone();
        // l'id lo decide il comando, così sa cosa rimuovere all'undo
        match &mut copia {
            Node::Layer(l) => l.id = self.new_id,
            Node::Group(g) => g.id = self.new_id,
        }
        self.created = doc.insert_node(self.parent, self.index, copia);
    }

    fn undo(&mut self, doc: &mut Document) {
        if self.created {
            doc.remove_node(self.new_id);
            self.created = false;
        }
    }

    fn label(&self) -> &str {
        "Duplica"
    }
}

/// Fonde il layer sopra in quello sotto, dentro lo stesso contenitore.
pub struct MergeDown {
    bottom: LayerId,
    top: LayerId,
    parent: Option<LayerId>,
    /// Il layer di sotto com'era prima della fusione, per l'undo.
    old_bottom: Option<Layer>,
    /// Il layer di sopra rimosso e la sua posizione, per l'undo.
    old_top: Option<(Node, usize)>,
}

impl MergeDown {
    pub fn new(bottom: LayerId, top: LayerId, parent: Option<LayerId>) -> Self {
        Self {
            bottom,
            top,
            parent,
            old_bottom: None,
            old_top: None,
        }
    }
}

impl Command for MergeDown {
    fn apply(&mut self, doc: &mut Document) {
        let Some(bottom) = doc.layer(self.bottom).cloned() else {
            return;
        };
        let Some(top_index) = doc.index_of(self.top) else {
            return;
        };
        let Some(top) = doc.node(self.top).cloned() else {
            return;
        };
        self.old_bottom = Some(bottom.clone());
        self.old_top = Some((top.clone(), top_index));

        // Il layer sotto si compone come base neutra (Normal, opacità 1, ma con
        // la sua maschera): così la maschera viene "cotta" nei pixel e non
        // verrebbe applicata due volte dopo la fusione.
        let mut base = bottom.clone();
        base.blend = BlendMode::Normal;
        base.opacity = 1.0;

        let fuso = compositor::composite_nodes(&[Node::Layer(base), top], doc.width, doc.height);

        let (doc_w, doc_h) = (doc.width, doc.height);
        doc.remove_node(self.top);

        if let Some(Node::Layer(sotto)) = doc.node_mut(self.bottom) {
            sotto.pixels = fuso;
            sotto.width = doc_w;
            sotto.height = doc_h;
            // i pixel sono ora nello spazio del documento
            sotto.transform = Transform::default();
            // blend e opacità del sotto continuano a valere verso i layer sotto;
            // la maschera è già stata applicata, quindi va tolta.
            sotto.mask = None;
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(vecchio) = self.old_bottom.take() {
            if let Some(nodo) = doc.node_mut(self.bottom) {
                *nodo = Node::Layer(vecchio);
            }
        }
        if let Some((top, index)) = self.old_top.take() {
            doc.insert_node(self.parent, index, top);
        }
    }

    fn label(&self) -> &str {
        "Fondi verso il basso"
    }
}

/// Sostituisce un gruppo con un layer che è il composito dei suoi figli.
pub struct FlattenGroup {
    group: LayerId,
    parent: Option<LayerId>,
    index: usize,
    width: u32,
    height: u32,
    old_group: Option<(Node, usize)>,
    new_layer: Option<LayerId>,
}

impl FlattenGroup {
    pub fn new(
        group: LayerId,
        parent: Option<LayerId>,
        index: usize,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            group,
            parent,
            index,
            width,
            height,
            old_group: None,
            new_layer: None,
        }
    }

    /// Applica e restituisce l'identificativo del nuovo layer.
    pub fn do_apply(&mut self, doc: &mut Document) -> Result<LayerId, LayerError> {
        let Some(gruppo) = doc.group(self.group).cloned() else {
            return Err(LayerError::NotAGroup);
        };
        let Some(index) = doc.index_of(self.group) else {
            return Err(LayerError::NotFound);
        };
        let nodo = Node::Group(gruppo.clone());
        self.old_group = Some((nodo, index));

        // Il composito del gruppo: figli, poi opacità e maschera del gruppo.
        let mut pixels = compositor::composite_nodes(&gruppo.children, self.width, self.height);
        compositor::scale_alpha(
            &mut pixels,
            self.width,
            self.height,
            gruppo.opacity,
            gruppo.mask.as_deref(),
        );

        let nuovo = Layer {
            id: LayerId::new(),
            stable: crate::stable::StableId::new(),
            name: gruppo.name.clone(),
            visible: gruppo.visible,
            width: self.width,
            height: self.height,
            pixels,
            transform: Transform::default(),
            blend: gruppo.blend,
            opacity: 1.0,
            mask: None,
        };
        self.new_layer = Some(nuovo.id);

        doc.remove_node(self.group);
        doc.insert_node(self.parent, index, Node::Layer(nuovo));
        Ok(self.new_layer.expect("assegnato sopra"))
    }
}

impl Command for FlattenGroup {
    fn apply(&mut self, doc: &mut Document) {
        let _ = self.do_apply(doc);
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(id) = self.new_layer.take() {
            doc.remove_node(id);
        }
        if let Some((nodo, _)) = self.old_group.take() {
            // la posizione salvata al momento dell'applicazione e' quella giusta
            doc.insert_node(self.parent, self.index, nodo);
        }
    }

    fn label(&self) -> &str {
        "Appiattisci gruppo"
    }
}

/// Sposta nodi fratelli in un nuovo gruppo.
pub struct CreateGroup {
    ids: Vec<LayerId>,
    parent: Option<LayerId>,
    name: String,
    new_group: Option<LayerId>,
    /// I nodi rimossi e le loro posizioni originali, per l'undo.
    removed: Vec<(Node, usize)>,
    insert_at: Option<usize>,
}

impl CreateGroup {
    pub fn new(ids: Vec<LayerId>, parent: Option<LayerId>, name: impl Into<String>) -> Self {
        Self {
            ids,
            parent,
            name: name.into(),
            new_group: None,
            removed: Vec::new(),
            insert_at: None,
        }
    }

    /// Applica e restituisce l'identificativo del gruppo creato.
    pub fn do_apply(&mut self, doc: &mut Document) -> Result<LayerId, LayerError> {
        let children = doc
            .children_of(self.parent)
            .ok_or(LayerError::InvalidTarget)?;
        // le posizioni originali, in ordine
        let mut posizioni: Vec<usize> = Vec::new();
        for id in &self.ids {
            let pos = children
                .iter()
                .position(|n| n.id() == *id)
                .ok_or(LayerError::NotSiblings)?;
            posizioni.push(pos);
        }
        let primo = *posizioni.iter().min().ok_or(LayerError::EmptySelection)?;

        let mut gruppo = Group::new(self.name.clone());
        for (id, pos) in self.ids.iter().zip(posizioni.iter()) {
            let nodo = doc.remove_node(*id).ok_or(LayerError::NotFound)?;
            self.removed.push((nodo.clone(), *pos));
            gruppo.children.push(nodo);
        }
        let id = gruppo.id;
        self.new_group = Some(id);
        self.insert_at = Some(primo);
        doc.insert_node(self.parent, primo, Node::Group(gruppo));
        Ok(id)
    }
}

impl Command for CreateGroup {
    fn apply(&mut self, doc: &mut Document) {
        let _ = self.do_apply(doc);
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(id) = self.new_group.take() {
            if let Some(Node::Group(g)) = doc.remove_node(id) {
                // i figli tornano dove erano, dal più basso al più alto
                let mut removed = std::mem::take(&mut self.removed);
                removed.sort_by_key(|(_, pos)| *pos);
                for (nodo, pos) in removed {
                    doc.insert_node(self.parent, pos, nodo);
                }
                let _ = g;
            }
        }
    }

    fn label(&self) -> &str {
        "Raggruppa"
    }
}

/// Scioglie un gruppo: i figli prendono il suo posto.
pub struct Ungroup {
    group: LayerId,
    parent: Option<LayerId>,
    index: usize,
    old_group: Option<Group>,
}

impl Ungroup {
    pub fn new(group: LayerId, parent: Option<LayerId>, index: usize) -> Self {
        Self {
            group,
            parent,
            index,
            old_group: None,
        }
    }

    pub fn do_apply(&mut self, doc: &mut Document) -> Result<(), LayerError> {
        let Some(gruppo) = doc.group(self.group).cloned() else {
            return Err(LayerError::NotAGroup);
        };
        let index = doc.index_of(self.group).ok_or(LayerError::NotFound)?;
        self.old_group = Some(gruppo.clone());
        doc.remove_node(self.group);
        for (i, figlio) in gruppo.children.into_iter().enumerate() {
            doc.insert_node(self.parent, index + i, figlio);
        }
        Ok(())
    }
}

impl Command for Ungroup {
    fn apply(&mut self, doc: &mut Document) {
        let _ = self.do_apply(doc);
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(gruppo) = self.old_group.take() {
            // rimuove i figli rimessi in circolazione, poi ricrea il gruppo
            for figlio in &gruppo.children {
                doc.remove_node(figlio.id());
            }
            doc.insert_node(self.parent, self.index, Node::Group(gruppo));
        }
    }

    fn label(&self) -> &str {
        "Separa"
    }
}

/// Sposta un nodo rispetto a un altro.
pub struct Reorder {
    node: LayerId,
    target: LayerId,
    insert: InsertWhere,
    old: Option<(Option<LayerId>, usize)>,
}

impl Reorder {
    pub fn new(node: LayerId, target: LayerId, insert: InsertWhere) -> Self {
        Self {
            node,
            target,
            insert,
            old: None,
        }
    }
}

impl Command for Reorder {
    fn apply(&mut self, doc: &mut Document) {
        let Some(parent) = doc.parent_of(self.node) else {
            return;
        };
        let Some(index) = doc.index_of(self.node) else {
            return;
        };
        self.old = Some((parent, index));

        let Some(nodo) = doc.remove_node(self.node) else {
            return;
        };
        // l'indice del target va ricalcolato **dopo** la rimozione: se il nodo
        // stava sotto il target, le posizioni sono scalate di uno.
        let Some(target_index) = doc.index_of(self.target) else {
            // il target era dentro il nodo rimosso: si ripristina
            doc.insert_node(parent, index, nodo);
            return;
        };
        match self.insert {
            InsertWhere::Before => {
                let dest = doc.parent_of(self.target).flatten();
                doc.insert_node(dest, target_index, nodo);
            }
            InsertWhere::After => {
                let dest = doc.parent_of(self.target).flatten();
                doc.insert_node(dest, target_index + 1, nodo);
            }
            InsertWhere::Inside => {
                doc.insert_node(Some(self.target), usize::MAX, nodo);
            }
        }
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some((parent, index)) = self.old.take() {
            if let Some(nodo) = doc.remove_node(self.node) {
                doc.insert_node(parent, index, nodo);
            }
        }
    }

    fn label(&self) -> &str {
        "Riordina"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_con_un_layer() -> (Document, LayerId) {
        let mut doc = Document::new(8, 8);
        let layer = Layer::filled("base", 8, 8, [255, 255, 255, 255]);
        let id = layer.id;
        doc.execute(Box::new(AddLayer::new(layer)));
        (doc, id)
    }

    #[test]
    fn aggiungi_e_annulla() {
        let (mut doc, id) = doc_con_un_layer();
        assert_eq!(doc.layer_count(), 1);
        assert!(doc.undo());
        assert!(doc.is_empty());
        assert!(doc.layer(id).is_none());
        assert!(doc.redo());
        assert_eq!(doc.layer_count(), 1);
    }

    #[test]
    fn aggiungi_in_posizione_precisa() {
        let mut doc = Document::new(4, 4);
        let sotto = Layer::filled("sotto", 4, 4, [1, 1, 1, 255]);
        let sopra = Layer::filled("sopra", 4, 4, [2, 2, 2, 255]);
        let sopra_id = sopra.id;
        doc.execute(Box::new(AddLayer::new(sotto)));
        doc.execute(Box::new(AddLayer::new(sopra).at(0)));
        assert_eq!(
            doc.layers()[0].id,
            sopra_id,
            "inserito in cima all'indice 0"
        );
        doc.undo();
        assert_eq!(doc.layer_count(), 1);
    }

    #[test]
    fn rimuovi_e_ripristina_nella_stessa_posizione() {
        let mut doc = Document::new(4, 4);
        let a = Layer::filled("a", 4, 4, [0, 0, 0, 255]);
        let b = Layer::filled("b", 4, 4, [0, 0, 0, 255]);
        let b_id = b.id;
        doc.execute(Box::new(AddLayer::new(a)));
        doc.execute(Box::new(AddLayer::new(b)));
        doc.execute(Box::new(RemoveLayer::new(b_id)));
        assert_eq!(doc.layer_count(), 1);
        doc.undo();
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.layers()[1].id, b_id, "tornato in fondo, dov'era");
    }

    #[test]
    fn opacita_undo_e_redo() {
        let (mut doc, id) = doc_con_un_layer();
        doc.execute(Box::new(SetOpacity::new(id, 0.25)));
        assert_eq!(doc.layer(id).unwrap().opacity, 0.25);
        doc.undo();
        assert_eq!(doc.layer(id).unwrap().opacity, 1.0);
        doc.redo();
        assert_eq!(doc.layer(id).unwrap().opacity, 0.25);
    }

    #[test]
    fn trasformazione_undo_e_redo() {
        let (mut doc, id) = doc_con_un_layer();
        let nuova = Transform::at(3.0, 4.0);
        doc.execute(Box::new(SetTransform::new(id, nuova)));
        assert_eq!(doc.layer(id).unwrap().transform.x, 3.0);
        doc.undo();
        assert_eq!(doc.layer(id).unwrap().transform.x, 0.0);
    }

    #[test]
    fn rinomina_e_blend_sono_reversibili() {
        let (mut doc, id) = doc_con_un_layer();
        doc.execute(Box::new(RenameLayer::new(id, "sfondo")));
        doc.execute(Box::new(SetBlendMode::new(id, BlendMode::Multiply)));
        assert_eq!(doc.layer(id).unwrap().name, "sfondo");
        assert_eq!(doc.layer(id).unwrap().blend, BlendMode::Multiply);
        doc.undo();
        doc.undo();
        assert_eq!(doc.layer(id).unwrap().name, "base");
        assert_eq!(doc.layer(id).unwrap().blend, BlendMode::Normal);
    }

    #[test]
    fn il_limite_scarta_i_comandi_piu_vecchi() {
        let mut doc = Document::new(4, 4);
        doc.history = History::new(3);
        for _ in 0..5 {
            doc.execute(Box::new(AddLayer::new(Layer::filled(
                "x",
                4,
                4,
                [0, 0, 0, 255],
            ))));
        }
        assert_eq!(doc.history.len(), 3);
        let mut annullati = 0;
        while doc.undo() {
            annullati += 1;
        }
        assert_eq!(annullati, 3, "solo i 3 più recenti sono annullabili");
        assert_eq!(doc.layer_count(), 2, "i due più vecchi restano");
    }

    #[test]
    fn un_nuovo_comando_azzera_i_redo() {
        let (mut doc, id) = doc_con_un_layer();
        doc.execute(Box::new(SetOpacity::new(id, 0.5)));
        doc.undo();
        assert!(doc.history.can_redo());
        doc.execute(Box::new(SetOpacity::new(id, 0.9)));
        assert!(
            !doc.history.can_redo(),
            "storia lineare: niente diramazioni"
        );
    }

    #[test]
    fn le_etichette_descrivono_la_storia() {
        let (mut doc, id) = doc_con_un_layer();
        doc.execute(Box::new(SetOpacity::new(id, 0.5)));
        assert_eq!(
            doc.history.labels(),
            vec!["Aggiungi livello", "Opacità livello"]
        );
    }

    #[test]
    fn undo_di_set_mask_ripristina_lo_stato() {
        let (mut doc, id) = doc_con_un_layer();
        assert!(doc.layer(id).unwrap().mask.is_none());

        let mask = Arc::new(Mask::full(8, 8));
        doc.execute(Box::new(SetLayerMask::new(id, Some(mask))));
        assert!(doc.layer(id).unwrap().mask.is_some());

        doc.undo();
        assert!(
            doc.layer(id).unwrap().mask.is_none(),
            "l'undo deve togliere la maschera"
        );

        doc.redo();
        assert!(doc.layer(id).unwrap().mask.is_some());
    }

    #[test]
    fn invertire_la_maschera_e_reversibile() {
        let (mut doc, id) = doc_con_un_layer();
        doc.execute(Box::new(SetLayerMask::new(
            id,
            Some(Arc::new(Mask::full(8, 8))),
        )));
        assert!(!doc.layer(id).unwrap().mask.as_ref().unwrap().inverted);

        doc.execute(Box::new(ToggleMaskInverted::new(id)));
        assert!(
            doc.layer(id).unwrap().mask.as_ref().unwrap().inverted,
            "la maschera deve essere invertita"
        );

        doc.undo();
        assert!(!doc.layer(id).unwrap().mask.as_ref().unwrap().inverted);
    }

    #[test]
    fn togliere_la_maschera_e_reversibile() {
        let (mut doc, id) = doc_con_un_layer();
        doc.execute(Box::new(SetLayerMask::new(
            id,
            Some(Arc::new(Mask::full(8, 8))),
        )));
        doc.execute(Box::new(SetLayerMask::new(id, None)));
        assert!(doc.layer(id).unwrap().mask.is_none());
        doc.undo();
        assert!(doc.layer(id).unwrap().mask.is_some());
    }
    #[test]
    fn annullare_a_vuoto_non_e_un_errore() {
        let mut doc = Document::new(2, 2);
        assert!(!doc.undo());
        assert!(!doc.redo());
    }
}
