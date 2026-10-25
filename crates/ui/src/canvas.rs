//! Conversioni fra lo stato dell'editor e i tipi di egui.

use egui::{Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions};

use crate::state::Editor;

/// Nome della texture del canvas.
pub const CANVAS_TEXTURE: &str = "canvas";

/// Carica (o ricarica) la texture del canvas dal composito.
pub fn aggiorna_texture(
    editor: &mut Editor,
    ctx: &egui::Context,
    texture: &mut Option<TextureHandle>,
) {
    let (rgba, w, h) = editor.composite_rgba();
    let immagine = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    match texture {
        Some(handle) => handle.set(immagine, TextureOptions::LINEAR),
        None => {
            *texture = Some(ctx.load_texture(CANVAS_TEXTURE, immagine, TextureOptions::LINEAR));
        }
    }
}

/// Rettangolo dello schermo occupato dal documento, dato zoom e pan.
pub fn rect_documento(area: Rect, doc_w: u32, doc_h: u32, zoom: f32, pan: egui::Vec2) -> Rect {
    let dimensione = egui::vec2(doc_w as f32 * zoom, doc_h as f32 * zoom);
    let centro = area.center() + pan;
    Rect::from_center_size(centro, dimensione)
}

/// Da punto sullo schermo a pixel del documento.
pub fn schermo_a_documento(
    punto: Pos2,
    rect: Rect,
    doc_w: u32,
    doc_h: u32,
) -> Option<(f32, f32)> {
    if !rect.contains(punto) {
        return None;
    }
    let u = (punto.x - rect.left()) / rect.width();
    let v = (punto.y - rect.top()) / rect.height();
    Some((u * doc_w as f32, v * doc_h as f32))
}

/// Applica uno zoom mantenendo fermo il punto sotto il cursore.
///
/// Restituisce `(zoom, pan)`. E' una funzione pura: la stessa che usa l'app,
/// così l'invariante ("il punto sotto il mouse non si muove") è testabile
/// senza finestra.
pub fn imposta_zoom(
    area: Rect,
    doc_w: u32,
    doc_h: u32,
    pan: (f32, f32),
    zoom_vecchio: f32,
    zoom_nuovo: f32,
    cursore: Pos2,
) -> (f32, (f32, f32)) {
    let before = rect_documento(area, doc_w, doc_h, zoom_vecchio, egui::vec2(pan.0, pan.1));
    if before.width() <= 0.0 || before.height() <= 0.0 {
        return (zoom_nuovo, pan);
    }
    // il punto sotto il cursore, in coordinate normalizzate del documento
    let u = (cursore.x - before.left()) / before.width();
    let v = (cursore.y - before.top()) / before.height();

    let dopo = rect_documento(area, doc_w, doc_h, zoom_nuovo, egui::vec2(pan.0, pan.1));
    let min_atteso = egui::pos2(
        cursore.x - u * dopo.width(),
        cursore.y - v * dopo.height(),
    );
    let delta = min_atteso - dopo.min;
    (zoom_nuovo, (pan.0 + delta.x, pan.1 + delta.y))
}

/// Colore del bordo di selezione.pub const BORDO_SELEZIONE: Color32 = Color32::from_rgb(255, 190, 60);
/// Colore del rettangolo di selezione in corso.
pub const SELEZIONE_IN_CORSO: Color32 = Color32::from_rgb(120, 200, 255);
