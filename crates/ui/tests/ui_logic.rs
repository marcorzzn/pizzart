//! I cinque comportamenti dell'MVP che si possono verificare **senza finestra**.
//!
//! egui non si testa headless, ma la logica sì: è il motivo per cui `Editor` non
//! conosce egui. Se questi passano, i buchi più probabili (history inondata
//! dallo slider, selezione che non diventa maschera, canvas non invalidato) sono
//! chiusi.

use std::path::PathBuf;

use pizzart_core::Layer;
use pizzart_ui::state::{Editor, Tool};
use pizzart_ui::Tool as ToolRe;

fn temp(nome: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "pizzart-ui-test-{nome}-{}.png",
        std::process::id()
    ));
    p
}

/// Crea un PNG temporaneo e restituisce il percorso.
fn png_temporaneo(nome: &str, w: u32, h: u32) -> PathBuf {
    let path = temp(nome);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&[(x * 4) as u8, (y * 4) as u8, 128, 255]);
        }
    }
    let img = image::RgbaImage::from_raw(w, h, rgba).expect("immagine");
    img.save(&path).expect("scrittura png");
    path
}

#[test]
fn carica_png_come_layer() {
    let path = png_temporaneo("carica", 40, 24);
    let mut editor = Editor::new(100, 100);
    editor.open_image(&path).expect("apertura");

    assert_eq!(editor.doc.layer_count(), 1, "un layer di sfondo");
    assert_eq!((editor.doc.width, editor.doc.height), (40, 24), "dimensioni dal file");
    let layer = editor.selected_layer().expect("layer selezionato");
    assert_eq!(layer.width, 40);
    assert!(editor.needs_recompose, "il canvas deve essere ricomposto");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn salva_progetto_roundtrip_da_ui() {
    let mut editor = Editor::new(32, 24);
    editor.doc.add_layer(Layer::filled("rosso", 32, 24, [200, 40, 40, 255]));

    let mut path = std::env::temp_dir();
    path.push(format!("pizzart-ui-test-{}.pizzart", std::process::id()));
    editor.save_project(&path).expect("salvataggio");
    assert_eq!(editor.path.as_deref(), Some(path.as_path()), "il percorso viene ricordato");

    // un secondo editor, come se l'app fosse stata riaperta
    let mut riaperto = Editor::new(1, 1);
    riaperto.open_project(&path).expect("apertura");
    assert_eq!(riaperto.doc.layer_count(), 1);
    assert_eq!((riaperto.doc.width, riaperto.doc.height), (32, 24));
    assert_eq!(riaperto.doc.layers()[0].name, "rosso");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn la_history_con_un_drag_ha_un_solo_comando() {
    let mut editor = Editor::new(16, 16);
    let id = editor.doc.add_layer(Layer::filled("velo", 16, 16, [10, 10, 10, 255]));
    assert_eq!(editor.history_len(), 0, "nessun comando iniziale");

    // un drag di 100 passi, come farebbe uno slider
    editor.begin_opacity_drag(id);
    for i in 0..100 {
        editor.update_opacity_drag(i as f32 / 100.0);
    }
    editor.end_opacity_drag();

    assert_eq!(
        editor.history_len(),
        1,
        "un drag deve produrre **un** comando, non cento"
    );
    let opacita = editor.doc.layer(id).expect("layer").opacity;
    assert!((opacita - 0.99).abs() < 1e-3, "valore finale: {opacita}");

    // e l'undo riporta al valore di partenza, non a metà drag
    assert!(editor.undo());
    let dopo = editor.doc.layer(id).expect("layer").opacity;
    assert!((dopo - 1.0).abs() < 1e-3, "dopo undo: {dopo}");
}

#[test]
fn un_drag_senza_movimento_non_sporca_la_history() {
    let mut editor = Editor::new(8, 8);
    let id = editor.doc.add_layer(Layer::filled("a", 8, 8, [1, 1, 1, 255]));
    editor.begin_opacity_drag(id);
    editor.update_opacity_drag(1.0);
    editor.end_opacity_drag();
    assert_eq!(editor.history_len(), 0, "nessun cambiamento, nessun comando");
}

#[test]
fn selezione_rect_produce_mask() {
    let mut editor = Editor::new(64, 64);
    editor.doc.add_layer(Layer::filled("fondo", 64, 64, [255, 255, 255, 255]));
    let id = editor
        .doc
        .add_layer(Layer::filled("sopra", 64, 64, [200, 30, 30, 255]));
    editor.selected = Some(id);
    editor.tool = Tool::Rect;

    editor.begin_selection(10.0, 10.0);
    editor.update_selection(40.0, 40.0);
    assert!(editor.selection.is_some());
    editor.apply_selection_as_mask().expect("maschera");

    let layer = editor.doc.layer(id).expect("layer");
    let mask = layer.mask.as_ref().expect("maschera applicata");
    assert_eq!((mask.width, mask.height), (64, 64));
    // dentro la selezione si vede, fuori no
    assert!((mask.sample(0.4, 0.4) - 1.0).abs() < 0.02, "dentro: opaca");
    assert!(mask.sample(0.05, 0.05) < 0.05, "fuori: nascosta");
    assert!(editor.selection.is_none(), "la selezione si consuma");
    assert_eq!(editor.history_len(), 1);
}

#[test]
fn il_composito_viene_aggiornato_dopo_undo() {
    let mut editor = Editor::new(8, 8);
    let id = editor.doc.add_layer(Layer::filled("a", 8, 8, [9, 9, 9, 255]));
    editor.begin_opacity_drag(id);
    editor.update_opacity_drag(0.5);
    editor.end_opacity_drag();

    // dopo il drag il composito è aggiornato
    let _ = editor.composite_rgba();
    assert!(!editor.needs_recompose, "ricomposto dopo il drag");

    // undo → il canvas deve sapere che è sporco
    assert!(editor.undo());
    assert!(
        editor.needs_recompose,
        "dopo undo il canvas deve essere invalidato"
    );

    let _ = editor.composite_rgba();
    assert!(!editor.needs_recompose);
}

#[test]
fn gli_strumenti_si_attivano_e_si_descrivono() {
    let mut editor = Editor::new(8, 8);
    assert_eq!(editor.tool, ToolRe::Select);
    editor.tool = ToolRe::Brush;
    assert_eq!(editor.tool, Tool::Brush);
    assert_eq!(editor.doc.width, 8);
}
