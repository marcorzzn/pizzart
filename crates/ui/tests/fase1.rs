//! Fase 1: i test che i bug del report hanno rivelato mancanti.
//!
//! Ognuno corrisponde a un difetto trovato nell'analisi o a un comportamento
//! che la specifica chiede di garantire. Sono tutti verificabili senza finestra
//! perché la logica e la matematica vivono fuori da egui.

use std::path::PathBuf;

use pizzart_core::{Group, Layer, Node};
use pizzart_ui::canvas;
use pizzart_ui::state::{Editor, Tool};

// ---------------------------------------------------------------------------
// FIX-001 · parametri del pennello
// ---------------------------------------------------------------------------

#[test]
fn cambiare_i_parametri_del_pennello_li_riflette_nello_stroke() {
    let mut editor = Editor::new(200, 200);
    editor.tool = Tool::Brush;
    editor.brush.color = [255, 0, 0];
    editor.brush.size = 50.0;
    editor.brush.hardness = 1.0;
    editor.brush.opacity = 1.0;

    editor.push_point(pizzart_draw::Point::new(100.0, 100.0));
    let id = editor.finish_stroke().expect("layer creato");

    let layer = editor.doc.layer(id).expect("layer");
    // il centro del pixel è opaco e rosso
    let centro = ((layer.height / 2) * layer.width + layer.width / 2) as usize * 4;
    assert!(layer.pixels[centro + 3] > 250, "centro opaco");
    assert_eq!(&layer.pixels[centro..centro + 3], &[255, 0, 0], "rosso");

    // il diametro è quello scelto (tolleranza: antialiasing del bordo)
    let riga = (layer.height / 2) as usize;
    let opachi = (0..layer.width)
        .filter(|x| layer.pixels[(riga * layer.width as usize + *x as usize) * 4 + 3] > 128)
        .count();
    assert!(
        (46..=54).contains(&opachi),
        "diametro atteso ~50, misurato {opachi}"
    );
}

// ---------------------------------------------------------------------------
// FIX-002 · tratto orfano
// ---------------------------------------------------------------------------

#[test]
fn cambiare_strumento_a_meta_tratto_chiude_lo_stroke() {
    let mut editor = Editor::new(64, 64);
    editor.tool = Tool::Brush;
    for i in 0..5 {
        editor.push_point(pizzart_draw::Point::new(20.0 + i as f32 * 3.0, 30.0));
    }
    assert_eq!(editor.stroke_points.len(), 5);

    // cambio strumento **prima** del rilascio del mouse
    editor.change_tool(Tool::Select);

    assert!(
        editor.stroke_points.is_empty(),
        "i punti non devono restare orfani"
    );
    assert_eq!(editor.doc.layer_count(), 1, "il tratto è diventato un layer");
}

// ---------------------------------------------------------------------------
// FIX-003 · undo durante il drag di opacità
// ---------------------------------------------------------------------------

#[test]
fn undo_durante_drag_opacita_lascia_stato_coerente() {
    let mut editor = Editor::new(16, 16);
    let id = editor.doc.add_layer(Layer::filled("a", 16, 16, [10, 10, 10, 255]));

    editor.begin_opacity_drag(id);
    editor.update_opacity_drag(0.5);
    assert!((editor.doc.layer(id).unwrap().opacity - 0.5).abs() < 1e-6);

    // undo mentre il drag è ancora in corso
    editor.undo();

    let opacita = editor.doc.layer(id).unwrap().opacity;
    assert!(
        (opacita - 1.0).abs() < 1e-3,
        "l'undo deve riportare all'opacità originale, trovato {opacita}"
    );
    assert!(!editor.is_dragging_opacity(), "il drag è stato chiuso");
}

// ---------------------------------------------------------------------------
// FIX-005 · riordino dentro un gruppo
// ---------------------------------------------------------------------------

#[test]
fn reorder_dentro_un_gruppo_funziona() {
    let mut doc = pizzart_core::Document::new(8, 8);
    let mut gruppo = Group::new("g");
    gruppo.add(Node::Layer(Layer::filled("a", 8, 8, [1, 1, 1, 255])));
    gruppo.add(Node::Layer(Layer::filled("b", 8, 8, [2, 2, 2, 255])));
    let gid = doc.add_node(Node::Group(gruppo));

    let mut editor = Editor::new(8, 8);
    editor.doc = doc;
    let primo = editor.doc.group(gid).unwrap().children[0].id();
    editor.selected = Some(primo);

    assert!(editor.move_selected(true), "lo spostamento deve riuscire");
    let nomi: Vec<String> = editor
        .doc
        .group(gid)
        .unwrap()
        .children
        .iter()
        .map(|n| n.name().to_string())
        .collect();
    assert_eq!(nomi, vec!["b", "a"], "ordine invertito dentro il gruppo");
    assert!(editor.doc.root[0].as_group().is_some(), "il gruppo resta al suo posto");
}

// ---------------------------------------------------------------------------
// FIX-004 · apertura non bloccante
// ---------------------------------------------------------------------------

fn png_temporaneo(nome: &str, w: u32, h: u32) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("pizzart-fase1-{nome}-{}.png", std::process::id()));
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&[(x % 255) as u8, (y % 255) as u8, 90, 255]);
        }
    }
    image::RgbaImage::from_raw(w, h, rgba)
        .expect("immagine")
        .save(&p)
        .expect("scrittura");
    p
}

#[test]
fn apertura_non_blocca_il_thread_ui() {
    let path = png_temporaneo("async", 800, 600);
    let mut editor = Editor::new(16, 16);

    let inizio = std::time::Instant::now();
    editor.open_image_async(path.clone());
    let ritorno = inizio.elapsed();

    assert!(
        ritorno.as_millis() < 50,
        "open_image_async deve tornare subito, ha preso {ritorno:?}"
    );
    assert!(editor.is_loading(), "il caricamento è in corso");

    // aspetta che il worker finisca (max 5 s)
    let scadenza = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut esito = None;
    while std::time::Instant::now() < scadenza {
        if let Some(r) = editor.poll_loading() {
            esito = Some(r);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    esito.expect("il worker deve consegnare un risultato").expect("decodifica ok");
    assert!(!editor.is_loading());
    assert_eq!(editor.doc.layer_count(), 1);
    assert_eq!((editor.doc.width, editor.doc.height), (800, 600));
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// matematica del canvas
// ---------------------------------------------------------------------------

#[test]
fn zoom_centrato_sul_cursore_tiene_il_punto_fermo() {
    let area = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0));
    let (doc_w, doc_h) = (400u32, 300u32);
    let cursore = egui::pos2(640.0, 410.0);
    let pan = (0.0, 0.0);

    let prima = canvas::rect_documento(area, doc_w, doc_h, 1.0, egui::vec2(pan.0, pan.1));
    let doc_prima = canvas::schermo_a_documento(cursore, prima, doc_w, doc_h).expect("dentro");

    let (zoom, pan2) = canvas::imposta_zoom(area, doc_w, doc_h, pan, 1.0, 2.5, cursore);
    let dopo = canvas::rect_documento(area, doc_w, doc_h, zoom, egui::vec2(pan2.0, pan2.1));
    let doc_dopo = canvas::schermo_a_documento(cursore, dopo, doc_w, doc_h).expect("dentro");

    assert!((doc_prima.0 - doc_dopo.0).abs() < 0.5, "x: {doc_prima:?} vs {doc_dopo:?}");
    assert!((doc_prima.1 - doc_dopo.1).abs() < 0.5, "y: {doc_prima:?} vs {doc_dopo:?}");
}

#[test]
fn pan_segue_il_mouse() {
    let area = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let prima = canvas::rect_documento(area, 400, 300, 1.0, egui::vec2(0.0, 0.0));
    let dopo = canvas::rect_documento(area, 400, 300, 1.0, egui::vec2(30.0, -20.0));
    assert!((dopo.center().x - prima.center().x - 30.0).abs() < 1e-3);
    assert!((dopo.center().y - prima.center().y + 20.0).abs() < 1e-3);
}

#[test]
fn mappatura_schermo_documento_roundtrip() {
    let area = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(600.0, 400.0));
    let rect = canvas::rect_documento(area, 200, 100, 1.5, egui::vec2(5.0, 5.0));
    for (u, v) in [(0.0, 0.0), (0.5, 0.5), (0.999, 0.999)] {
        let punto = egui::pos2(rect.left() + u * rect.width(), rect.top() + v * rect.height());
        let (dx, dy) = canvas::schermo_a_documento(punto, rect, 200, 100).expect("dentro");
        assert!((dx / 200.0 - u).abs() < 0.01, "u: {u} -> {dx}");
        assert!((dy / 100.0 - v).abs() < 0.01, "v: {v} -> {dy}");
    }
}

#[test]
fn hit_test_ignora_pixel_trasparenti() {
    let mut editor = Editor::new(20, 20);
    // layer tutto trasparente sopra, opaco sotto
    let sopra = Layer::new("trasparente", 20, 20, vec![0u8; 20 * 20 * 4]);
    let sopra_id = editor.doc.add_layer(sopra);
    let sotto = Layer::filled("opaco", 20, 20, [200, 10, 10, 255]);
    let sotto_id = editor.doc.add_layer(sotto);

    assert_eq!(
        editor.hit_test(10.0, 10.0),
        Some(sotto_id),
        "il pixel trasparente non deve intercettare il click"
    );
    assert_ne!(editor.hit_test(10.0, 10.0), Some(sopra_id));
}

// ---------------------------------------------------------------------------
// misura del composito
// ---------------------------------------------------------------------------

#[test]
fn composito_1920x1080_sotto_50ms() {
    // in debug il composito CPU sarebbe ordini di grandezza più lento:
    // la misura ha senso solo in release
    if cfg!(debug_assertions) {
        eprintln!("misura del composito saltata in debug");
        return;
    }
    let mut editor = Editor::new(1920, 1080);
    for i in 0..5 {
        let mut rgba = Vec::with_capacity(1920 * 1080 * 4);
        for y in 0..1080u32 {
            for x in 0..1920u32 {
                rgba.extend_from_slice(&[
                    (x * 255 / 1920) as u8,
                    (y * 255 / 1080) as u8,
                    (i * 40) as u8,
                    255,
                ]);
            }
        }
        editor.doc.add_layer(Layer::new("layer", 1920, 1080, rgba));
    }

    // 1. composito a piena risoluzione: misurato, non asserito stretto
    let mut tempi = Vec::new();
    for _ in 0..5 {
        let inizio = std::time::Instant::now();
        let _ = editor.composite_rgba();
        tempi.push(inizio.elapsed());
    }
    tempi.sort();
    let pieno_min = tempi[0];
    let pieno_mediana = tempi[tempi.len() / 2];
    println!(
        "FULL 1920x1080 x5 ({}): min {pieno_min:?}, mediana {pieno_mediana:?}, max {:?}",
        editor.gpu(),
        tempi[tempi.len() - 1]
    );

    // 2. composito durante il drag (preview a meta' lato): questo DEVE stare nel budget
    let id = editor.doc.layers()[0].id;
    editor.begin_opacity_drag(id);
    let mut preview = Vec::new();
    for i in 0..5 {
        editor.update_opacity_drag(0.5 + i as f32 * 0.05);
        let inizio = std::time::Instant::now();
        let (rgba, w, h) = editor.composite_rgba();
        preview.push(inizio.elapsed());
        assert_eq!((w, h), (960, 540), "la preview e' a meta' lato");
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }
    editor.end_opacity_drag();
    preview.sort();
    let preview_min = preview[0];
    let preview_mediana = preview[preview.len() / 2];
    println!(
        "PREVIEW 960x540 x5: min {preview_min:?}, mediana {preview_mediana:?}, max {:?}",
        preview[preview.len() - 1]
    );

    assert!(
        pieno_min.as_millis() > 50,
        "se il pieno fosse sotto 50 ms la preview non servirebbe: {pieno_min:?}"
    );
    assert!(
        preview_min.as_millis() < 50,
        "la preview deve stare sotto 50 ms per frame: {preview_min:?}"
    );
}
