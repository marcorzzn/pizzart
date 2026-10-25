//! Operazioni sui layer: fusione, appiattimento, duplicazione, raggruppamento,
//! separazione, riordino. Tutto attraverso l'API pubblica: se questi test
//! passano, l'API è usabile da una UI.

use std::sync::Arc;

use pizzart_core::{
    BlendMode, Document, Group, InsertWhere, Layer, LayerError, LayerId, Mask, MaskSource, Node,
};

fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y as usize * w as usize) + x as usize) * 4;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

fn nomi(doc: &Document) -> Vec<String> {
    doc.layers().iter().map(|l| l.name.clone()).collect()
}

fn radice(doc: &Document) -> Vec<String> {
    doc.root.iter().map(|n| n.name().to_string()).collect()
}

#[test]
fn merge_down_fonde_due_layer() {
    let mut doc = Document::new(8, 8);
    let sotto = doc.add_layer(Layer::filled("sotto", 8, 8, [255, 255, 255, 255]));
    let mut sopra = Layer::filled("sopra", 8, 8, [200, 40, 40, 255]);
    sopra.blend = BlendMode::Multiply;
    sopra.opacity = 0.5;
    let sopra_id = doc.add_layer(sopra);

    let prima = doc.composite();
    doc.merge_down(sopra_id).expect("fusione");
    let dopo = doc.composite();

    assert_eq!(doc.layer_count(), 1, "resta un solo layer");
    assert_eq!(doc.layer(sotto).unwrap().name, "sotto", "il risultato è il layer sotto");
    assert_eq!(prima, dopo, "il composito non deve cambiare");
    // il colore è quello atteso: multiply al 50% su bianco
    assert_eq!(px(&dopo, 8, 0, 0), [228, 148, 148, 255]);
}

#[test]
fn merge_down_su_layer_piu_basso_e_errore() {
    let mut doc = Document::new(4, 4);
    let fondo = doc.add_layer(Layer::filled("fondo", 4, 4, [10, 10, 10, 255]));
    doc.add_layer(Layer::filled("sopra", 4, 4, [200, 200, 200, 255]));

    assert_eq!(doc.merge_down(fondo), Err(LayerError::CannotMergeDown));
    assert_eq!(doc.layer_count(), 2, "il documento non deve cambiare");
}

#[test]
fn merge_down_con_mask() {
    let mut doc = Document::new(4, 4);
    doc.add_layer(Layer::filled("fondo", 4, 4, [20, 40, 200, 255]));
    let mut sopra = Layer::filled("sopra", 4, 4, [240, 240, 60, 255]);
    sopra.mask = Some(Arc::new(Mask::new(
        vec![128; 16],
        4,
        4,
        MaskSource::Procedural,
    )));
    let sopra_id = doc.add_layer(sopra);

    let prima = doc.composite();
    doc.merge_down(sopra_id).expect("fusione");
    let dopo = doc.composite();

    assert_eq!(prima, dopo, "la maschera deve essere cotta nel risultato");
}

#[test]
fn flatten_group_produce_un_layer_alla_posizione_del_gruppo() {
    let mut doc = Document::new(4, 4);
    doc.add_layer(Layer::filled("fondo", 4, 4, [0, 0, 0, 255]));
    let mut g = Group::new("gruppo");
    g.add(Node::Layer(Layer::filled("a", 4, 4, [255, 0, 0, 255])));
    g.add(Node::Layer(Layer::filled("b", 4, 4, [0, 0, 255, 255])));
    let gid = doc.add_node(Node::Group(g));
    doc.add_layer(Layer::filled("cima", 4, 4, [10, 200, 10, 255]));

    let prima = doc.composite();
    let nuovo = doc.flatten_group(gid).expect("appiattimento");
    let dopo = doc.composite();

    assert_eq!(prima, dopo, "il composito non cambia");
    assert!(doc.group(gid).is_none(), "il gruppo non esiste più");
    assert_eq!(radice(&doc), vec!["fondo", "gruppo", "cima"], "stessa posizione");
    let layer = doc.layer(nuovo).expect("nuovo layer");
    assert_eq!(layer.width, 4, "il layer ha la dimensione del documento");
    assert_eq!(layer.opacity, 1.0, "opacità già applicata");
    assert_eq!(layer.blend, BlendMode::Normal, "blend già applicato");
}

#[test]
fn flatten_group_con_mask() {
    let mut doc = Document::new(4, 4);
    doc.add_layer(Layer::filled("fondo", 4, 4, [0, 0, 0, 255]));
    let mut g = Group::new("gruppo");
    g.add(Node::Layer(Layer::filled("rosso", 4, 4, [255, 0, 0, 255])));
    g.mask = Some(Arc::new(Mask::new(vec![128; 16], 4, 4, MaskSource::Procedural)));
    let gid = doc.add_node(Node::Group(g));

    let prima = doc.composite();
    doc.flatten_group(gid).expect("appiattimento");
    let dopo = doc.composite();

    assert_eq!(prima, dopo, "la maschera del gruppo è cotta nel risultato");
}

#[test]
fn duplicate_clona_mask_ma_non_la_condivide() {
    let mut doc = Document::new(4, 4);
    let mut l = Layer::filled("originale", 4, 4, [120, 200, 90, 255]);
    l.mask = Some(Arc::new(Mask::full(4, 4)));
    let id = doc.add_layer(l);

    let copia = doc.duplicate(id).expect("duplicato");
    assert_eq!(doc.layer_count(), 2);
    assert_eq!(doc.index_of(copia), doc.index_of(id).map(|i| i + 1), "sopra l'originale");

    let (a, b) = (
        doc.layer(id).unwrap().mask.clone().unwrap(),
        doc.layer(copia).unwrap().mask.clone().unwrap(),
    );
    assert!(
        !Arc::ptr_eq(&a, &b),
        "le maschere non devono essere condivise"
    );

    // invertire quella dell'originale non deve toccare la copia
    doc.node_mut(id);
    let invertita = Arc::new((*a).clone().with_inverted(true));
    doc.layer_mut(id).unwrap().mask = Some(invertita);
    assert!(
        !doc.layer(copia).unwrap().mask.as_ref().unwrap().inverted,
        "la copia resta invariata"
    );
}

#[test]
fn reorder_before_after_inside() {
    let mut doc = Document::new(2, 2);
    let a = doc.add_layer(Layer::filled("a", 2, 2, [1, 0, 0, 255]));
    let b = doc.add_layer(Layer::filled("b", 2, 2, [2, 0, 0, 255]));
    let c = doc.add_layer(Layer::filled("c", 2, 2, [3, 0, 0, 255]));

    // Before: a finisce sotto b
    doc.reorder(a, b, InsertWhere::Before).expect("before");
    assert_eq!(nomi(&doc), vec!["a", "b", "c"]);

    // After: a finisce sopra b
    doc.reorder(a, b, InsertWhere::After).expect("after");
    assert_eq!(nomi(&doc), vec!["b", "a", "c"]);

    // Inside: c entra nel gruppo
    let mut g = Group::new("gruppo");
    g.add(Node::Layer(Layer::filled("dentro", 2, 2, [9, 0, 0, 255])));
    let gid = doc.add_node(Node::Group(g));
    doc.reorder(c, gid, InsertWhere::Inside).expect("inside");
    assert_eq!(nomi(&doc), vec!["b", "a", "dentro", "c"]);
    assert_eq!(doc.group(gid).unwrap().children.len(), 2);
}

#[test]
fn reorder_inside_non_gruppo_e_errore() {
    let mut doc = Document::new(2, 2);
    let a = doc.add_layer(Layer::filled("a", 2, 2, [1, 0, 0, 255]));
    let b = doc.add_layer(Layer::filled("b", 2, 2, [2, 0, 0, 255]));
    assert_eq!(
        doc.reorder(a, b, InsertWhere::Inside),
        Err(LayerError::InvalidTarget)
    );
    assert_eq!(nomi(&doc), vec!["a", "b"], "niente è cambiato");
}

#[test]
fn create_group_sposta_i_layer_indicati() {
    let mut doc = Document::new(2, 2);
    doc.add_layer(Layer::filled("fondo", 2, 2, [0, 0, 0, 255]));
    let a = doc.add_layer(Layer::filled("a", 2, 2, [1, 0, 0, 255]));
    let b = doc.add_layer(Layer::filled("b", 2, 2, [2, 0, 0, 255]));

    let gid = doc.create_group(&[a, b]).expect("gruppo");
    assert_eq!(doc.layer_count(), 3, "nessun layer perso");
    assert_eq!(radice(&doc), vec!["fondo", "Gruppo"]);
    assert_eq!(
        doc.group(gid).unwrap().children.len(),
        2,
        "i due layer sono dentro il gruppo"
    );
    assert_eq!(doc.parent_of(a), Some(Some(gid)));
    assert_eq!(doc.parent_of(b), Some(Some(gid)));
}

#[test]
fn create_group_non_fratelli_e_errore() {
    let mut doc = Document::new(2, 2);
    let a = doc.add_layer(Layer::filled("a", 2, 2, [1, 0, 0, 255]));
    let mut g = Group::new("gruppo");
    g.add(Node::Layer(Layer::filled("dentro", 2, 2, [2, 0, 0, 255])));
    let gid = doc.add_node(Node::Group(g));
    let dentro = doc.group(gid).unwrap().children[0].id();

    assert_eq!(
        doc.create_group(&[a, dentro]),
        Err(LayerError::NotSiblings)
    );
}

#[test]
fn ungroup_ripristina_i_figli() {
    let mut doc = Document::new(2, 2);
    doc.add_layer(Layer::filled("fondo", 2, 2, [0, 0, 0, 255]));
    let a = doc.add_layer(Layer::filled("a", 2, 2, [1, 0, 0, 255]));
    let b = doc.add_layer(Layer::filled("b", 2, 2, [2, 0, 0, 255]));
    let gid = doc.create_group(&[a, b]).expect("gruppo");

    doc.ungroup(gid).expect("separazione");
    assert!(doc.group(gid).is_none());
    assert_eq!(nomi(&doc), vec!["fondo", "a", "b"], "i figli al posto del gruppo");
    assert_eq!(doc.parent_of(a), Some(None), "i figli tornano alla radice");
}

#[test]
fn undo_merge_down_ripristina_i_due_layer() {
    let mut doc = Document::new(4, 4);
    doc.add_layer(Layer::filled("fondo", 4, 4, [30, 30, 30, 255]));
    let sopra = doc.add_layer(Layer::filled("sopra", 4, 4, [200, 100, 50, 255]));
    let prima = doc.composite();

    doc.merge_down(sopra).expect("fusione");
    assert_eq!(doc.layer_count(), 1, "dopo la fusione resta un solo layer");

    doc.undo();
    assert_eq!(doc.layer_count(), 2, "i due layer sono tornati");
    assert_eq!(doc.composite(), prima, "e il composito è quello di prima");
}

#[test]
fn undo_flatten_ripristina_il_gruppo() {
    let mut doc = Document::new(4, 4);
    doc.add_layer(Layer::filled("fondo", 4, 4, [0, 0, 0, 255]));
    let mut g = Group::new("gruppo");
    g.add(Node::Layer(Layer::filled("a", 4, 4, [255, 0, 0, 255])));
    g.add(Node::Layer(Layer::filled("b", 4, 4, [0, 255, 0, 255])));
    let gid = doc.add_node(Node::Group(g));
    let prima = doc.composite();

    doc.flatten_group(gid).expect("appiattimento");
    doc.undo();

    assert!(doc.group(gid).is_some(), "il gruppo è tornato");
    assert_eq!(doc.group(gid).unwrap().children.len(), 2);
    assert_eq!(doc.composite(), prima);
}

#[test]
fn gruppo_nidificato() {
    let mut doc = Document::new(4, 4);
    doc.add_layer(Layer::filled("fondo", 4, 4, [0, 0, 255, 255]));

    // gruppo interno: rosso al 50%
    let mut interno = Group::new("interno");
    let mut rosso = Layer::filled("rosso", 4, 4, [255, 0, 0, 255]);
    rosso.opacity = 0.5;
    interno.add(Node::Layer(rosso));

    // gruppo esterno che contiene quello interno
    let mut esterno = Group::new("esterno");
    esterno.add(Node::Group(interno));
    doc.add_node(Node::Group(esterno));

    let out = doc.composite();
    // rosso al 50% su blu: metà rosso, metà blu
    let p = px(&out, 4, 1, 1);
    assert!((p[0] as i32 - 128).abs() <= 2, "R: {}", p[0]);
    assert!(p[1] <= 2, "G: {}", p[1]);
    assert!((p[2] as i32 - 128).abs() <= 2, "B: {}", p[2]);
    assert_eq!(p[3], 255);
}

#[test]
fn il_documento_composto_e_identico_prima_e_dopo_merge_seguito_da_undo() {
    let mut doc = Document::new(6, 6);
    doc.add_layer(Layer::filled("a", 6, 6, [240, 220, 30, 255]));
    let mut b = Layer::filled("b", 6, 6, [30, 90, 200, 255]);
    b.blend = BlendMode::Screen;
    let bid = doc.add_layer(b);

    let prima = doc.composite();
    doc.merge_down(bid).expect("fusione");
    doc.undo();
    let dopo = doc.composite();

    assert_eq!(prima, dopo);
    assert_eq!(doc.layer_count(), 2);
}

#[test]
fn reorder_e_reversibile() {
    let mut doc = Document::new(2, 2);
    let a = doc.add_layer(Layer::filled("a", 2, 2, [1, 0, 0, 255]));
    let b = doc.add_layer(Layer::filled("b", 2, 2, [2, 0, 0, 255]));
    doc.reorder(a, b, InsertWhere::After).expect("after");
    assert_eq!(nomi(&doc), vec!["b", "a"]);
    doc.undo();
    assert_eq!(nomi(&doc), vec!["a", "b"]);
}

#[test]
fn un_gruppo_nascosto_non_contribuisce() {
    let mut doc = Document::new(2, 2);
    doc.add_layer(Layer::filled("fondo", 2, 2, [255, 255, 255, 255]));
    let mut g = Group::new("nascosto");
    g.visible = false;
    g.add(Node::Layer(Layer::filled("rosso", 2, 2, [255, 0, 0, 255])));
    doc.add_node(Node::Group(g));

    assert_eq!(px(&doc.composite(), 2, 0, 0), [255, 255, 255, 255]);
}

#[test]
fn l_opacita_del_gruppo_si_applica_al_composito_dei_figli() {
    let mut doc = Document::new(2, 2);
    doc.add_layer(Layer::filled("fondo", 2, 2, [0, 0, 0, 255]));
    let mut g = Group::new("gruppo");
    g.opacity = 0.5;
    g.add(Node::Layer(Layer::filled("bianco", 2, 2, [255, 255, 255, 255])));
    doc.add_node(Node::Group(g));

    let p = px(&doc.composite(), 2, 0, 0);
    assert!(
        (p[0] as i32 - 128).abs() <= 2,
        "bianco al 50% su nero deve dare grigio: {p:?}"
    );
}

#[test]
fn un_id_sconosciuto_da_not_found() {
    let mut doc = Document::new(2, 2);
    let sconosciuto = LayerId::new();
    assert_eq!(doc.merge_down(sconosciuto), Err(LayerError::NotFound));
    assert_eq!(doc.flatten_group(sconosciuto), Err(LayerError::NotFound));
    assert_eq!(doc.ungroup(sconosciuto), Err(LayerError::NotFound));
    assert_eq!(
        doc.create_group(&[]),
        Err(LayerError::EmptySelection),
        "selezione vuota"
    );
}
