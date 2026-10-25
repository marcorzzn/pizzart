//! L'albero del documento: un nodo è un layer o un gruppo.
//!
//! Il documento è passato da `Vec<Layer>` a un albero perché un gruppo non è
//! "un layer speciale": ha figli, si compone in un buffer temporaneo, e applica
//! blend/opacità/maschera **al risultato dei figli**. Tenerlo come campo di un
//! layer avrebbe significato un `Option` e un sacco di `if`.
//!
//! Il gruppo applica sempre i propri attributi: niente "passa attraverso"
//! (Photoshop), che renderebbe la composizione dipendente dall'ordine dei
//! fratelli. Se servirà un caso d'uso reale, si aggiungerà con un test.

use std::sync::Arc;

use crate::blend::BlendMode;
use crate::layer::{Layer, LayerId};
use crate::mask::Mask;
use crate::stable::StableId;

/// Un nodo dell'albero: un layer con pixel, o un gruppo con figli.
#[derive(Clone, Debug)]
pub enum Node {
    Layer(Layer),
    Group(Group),
}

impl Node {
    pub fn id(&self) -> LayerId {
        match self {
            Node::Layer(l) => l.id,
            Node::Group(g) => g.id,
        }
    }

    /// Identificativo stabile nel file.
    pub fn stable(&self) -> StableId {
        match self {
            Node::Layer(l) => l.stable,
            Node::Group(g) => g.stable,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Node::Layer(l) => &l.name,
            Node::Group(g) => &g.name,
        }
    }

    pub fn is_visible(&self) -> bool {
        match self {
            Node::Layer(l) => l.visible,
            Node::Group(g) => g.visible,
        }
    }

    pub fn as_layer(&self) -> Option<&Layer> {
        match self {
            Node::Layer(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_layer_mut(&mut self) -> Option<&mut Layer> {
        match self {
            Node::Layer(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_group(&self) -> Option<&Group> {
        match self {
            Node::Group(g) => Some(g),
            _ => None,
        }
    }

    pub fn as_group_mut(&mut self) -> Option<&mut Group> {
        match self {
            Node::Group(g) => Some(g),
            _ => None,
        }
    }

    /// Numero di layer contenuti (un gruppo conta i suoi figli).
    pub fn layer_count(&self) -> usize {
        match self {
            Node::Layer(_) => 1,
            Node::Group(g) => count_layers(&g.children),
        }
    }

    /// Copia profonda con **identificativi nuovi** e maschere clone.
    ///
    /// La maschera va clonata davvero (`Arc::new((**m).clone())`): condividere
    /// l'`Arc` significherebbe che invertire la maschera dell'originale cambia
    /// anche il duplicato — e l'undo di quel cambio toccherebbe due layer.
    pub fn deep_clone(&self) -> Node {
        match self {
            Node::Layer(l) => {
                let mut copia = l.clone();
                copia.id = LayerId::new();
                copia.mask = l.mask.as_ref().map(|m| Arc::new((**m).clone()));
                Node::Layer(copia)
            }
            Node::Group(g) => Node::Group(Group {
                id: LayerId::new(),
                stable: StableId::new(),
                name: format!("{} copia", g.name),
                visible: g.visible,
                opacity: g.opacity,
                blend: g.blend,
                mask: g.mask.as_ref().map(|m| Arc::new((**m).clone())),
                children: g.children.iter().map(|c| c.deep_clone()).collect(),
            }),
        }
    }
}

/// Dove inserire un nodo rispetto a un altro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertWhere {
    /// Subito prima del nodo indicato (sotto, nello stack).
    Before,
    /// Subito dopo il nodo indicato (sopra).
    After,
    /// Dentro il nodo indicato, che deve essere un gruppo.
    Inside,
}

/// Un gruppo: figli composti insieme, poi blend/opacità/maschera applicati al
/// risultato.
#[derive(Clone, Debug)]
pub struct Group {
    pub id: LayerId,
    /// Identificativo stabile nel file (diverso da id, che e' un contatore).
    pub stable: StableId,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    /// Applicato **al composito dei figli**, non a ogni figlio.
    pub blend: BlendMode,
    pub mask: Option<Arc<Mask>>,
    /// Dal fondo alla cima, come il documento.
    pub children: Vec<Node>,
}

impl Group {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: LayerId::new(),
            stable: StableId::new(),
            name: name.into(),
            visible: true,
            opacity: 1.0,
            blend: BlendMode::Normal,
            mask: None,
            children: Vec::new(),
        }
    }

    pub fn add(&mut self, node: Node) -> &mut Self {
        self.children.push(node);
        self
    }

    pub fn layer_count(&self) -> usize {
        count_layers(&self.children)
    }
}

/// Cerca un nodo per identificativo, in tutto l'albero.
pub fn find(nodes: &[Node], id: LayerId) -> Option<&Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Node::Group(g) = node {
            if let Some(found) = find(&g.children, id) {
                return Some(found);
            }
        }
    }
    None
}

/// Cerca un nodo mutabile, in tutto l'albero.
pub fn find_mut(nodes: &mut [Node], id: LayerId) -> Option<&mut Node> {
    for node in nodes.iter_mut() {
        if node.id() == id {
            return Some(node);
        }
        if let Node::Group(g) = node {
            if let Some(found) = find_mut(&mut g.children, id) {
                return Some(found);
            }
        }
    }
    None
}

/// Il contenitore che ospita il nodo: `Some(None)` per la radice,
/// `None` se il nodo non esiste.
pub fn parent_of(nodes: &[Node], id: LayerId) -> Option<Option<LayerId>> {
    for node in nodes {
        if node.id() == id {
            return Some(None);
        }
        if let Node::Group(g) = node {
            if let Some(inner) = parent_of(&g.children, id) {
                // il figlio diretto del gruppo ha questo gruppo come genitore
                return Some(inner.or(Some(g.id)));
            }
        }
    }
    None
}

/// Posizione del nodo fra i suoi fratelli, **a questo livello** dell'albero.
pub fn position_of(nodes: &[Node], id: LayerId) -> Option<usize> {
    nodes.iter().position(|n| n.id() == id)
}

/// Il vettore di nodi di un contenitore (radice o gruppo).
pub fn container_mut(nodes: &mut Vec<Node>, parent: Option<LayerId>) -> Option<&mut Vec<Node>> {
    match parent {
        None => Some(nodes),
        Some(id) => match find_mut(nodes, id)? {
            Node::Group(g) => Some(&mut g.children),
            Node::Layer(_) => None,
        },
    }
}

/// Rimuove il nodo dall'albero e lo restituisce.
pub fn remove(nodes: &mut Vec<Node>, id: LayerId) -> Option<Node> {
    for i in 0..nodes.len() {
        if nodes[i].id() == id {
            return Some(nodes.remove(i));
        }
        if let Node::Group(g) = &mut nodes[i] {
            if let Some(found) = remove(&mut g.children, id) {
                return Some(found);
            }
        }
    }
    None
}

/// Inserisce un nodo nel contenitore indicato, con clamping sull'indice.
pub fn insert_into(
    nodes: &mut Vec<Node>,
    parent: Option<LayerId>,
    index: usize,
    node: Node,
) -> bool {
    match container_mut(nodes, parent) {
        Some(children) => {
            let index = index.min(children.len());
            children.insert(index, node);
            true
        }
        None => false,
    }
}

/// Tutti i layer, in ordine di disegno (dal fondo alla cima), appiattendo i gruppi.
pub fn flatten<'a>(nodes: &'a [Node], out: &mut Vec<&'a Layer>) {
    for node in nodes {
        match node {
            Node::Layer(l) => out.push(l),
            Node::Group(g) => flatten(&g.children, out),
        }
    }
}

/// Numero di layer, ricorsivo.
pub fn count_layers(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Layer(_) => 1,
            Node::Group(g) => count_layers(&g.children),
        })
        .sum()
}

/// Tutti gli identificativi dell'albero, ricorsivo (utile nei test e nella UI).
pub fn ids(nodes: &[Node], out: &mut Vec<LayerId>) {
    for node in nodes {
        out.push(node.id());
        if let Node::Group(g) = node {
            ids(&g.children, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(name: &str) -> Node {
        Node::Layer(Layer::filled(name, 2, 2, [10, 20, 30, 255]))
    }

    #[test]
    fn un_gruppo_conta_i_suoi_figli() {
        let mut g = Group::new("gruppo");
        g.add(layer("a"));
        g.add(layer("b"));
        assert_eq!(g.layer_count(), 2);
        let nodo = Node::Group(g);
        assert_eq!(nodo.layer_count(), 2);
        assert!(nodo.as_group().is_some());
        assert!(nodo.as_layer().is_none());
    }

    #[test]
    fn la_ricerca_entra_nei_gruppi() {
        let interno = layer("interno");
        let id = interno.id();
        let mut g = Group::new("g");
        let gid = g.id;
        g.add(interno);
        let albero = vec![Node::Group(g), layer("cima")];

        assert!(find(&albero, id).is_some());
        assert_eq!(
            parent_of(&albero, id),
            Some(Some(gid)),
            "il genitore è il gruppo"
        );
        assert_eq!(
            parent_of(&albero, gid),
            Some(None),
            "il gruppo è alla radice"
        );
        assert_eq!(parent_of(&albero, LayerId::new()), None, "id sconosciuto");
    }

    #[test]
    fn deep_clone_cambia_gli_identificativi_e_clona_la_maschera() {
        let mut l = Layer::filled("a", 2, 2, [1, 2, 3, 255]);
        l.mask = Some(Arc::new(Mask::full(2, 2)));
        let originale = Node::Layer(l);
        let copia = originale.deep_clone();

        assert_ne!(originale.id(), copia.id(), "identificativo nuovo");
        let (a, b) = (
            originale.as_layer().unwrap().mask.as_ref().unwrap(),
            copia.as_layer().unwrap().mask.as_ref().unwrap(),
        );
        assert!(!Arc::ptr_eq(a, b), "la maschera non deve essere condivisa");
    }

    #[test]
    fn remove_trova_anche_i_nodi_annidati() {
        let interno = layer("interno");
        let id = interno.id();
        let mut g = Group::new("g");
        g.add(interno);
        let mut albero = vec![Node::Group(g)];
        assert!(remove(&mut albero, id).is_some());
        assert_eq!(count_layers(&albero), 0);
    }

    #[test]
    fn insert_into_la_radice_e_i_gruppi() {
        let g = Group::new("g");
        let gid = g.id;
        let mut albero = vec![Node::Group(g)];
        assert!(insert_into(&mut albero, Some(gid), 0, layer("dentro")));
        assert!(insert_into(&mut albero, None, 99, layer("cima")));
        assert_eq!(count_layers(&albero), 2);
        assert_eq!(albero.len(), 2);
    }
}
