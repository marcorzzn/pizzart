//! Compositing su CPU: il riferimento che la GPU dovrà eguagliare.
//!
//! Questo modulo produce l'immagine finale sommando i layer dal fondo alla cima,
//! applicando trasformazione, opacità e blend mode. È volutamente lento e
//! semplice: serve come **verità di riferimento** nei test e per l'export
//! quando non c'è una GPU (CI, server, macchine virtuali).

use crate::blend::{blend_px, BlendMode};
use crate::document::Document;
use crate::layer::Layer;
use crate::mask::Mask;
use crate::node::Node;

/// Compone l'albero del documento in un buffer RGBA8 delle sue dimensioni.
pub fn composite(doc: &Document) -> Vec<u8> {
    composite_nodes(&doc.root, doc.width, doc.height)
}

/// Compone una lista di nodi in un buffer RGBA8.
///
/// Un gruppo viene composto **prima in un buffer temporaneo**, poi opacità e
/// maschera ne scalano l'alpha e il blend del gruppo lo fonde con il resto.
/// Il costo è un buffer pieno per ogni gruppo: su una tela 4000×4000 sono
/// 64 MB per livello di annidamento. Documentato, non ottimizzato.
pub fn composite_nodes(nodes: &[Node], doc_w: u32, doc_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; doc_w as usize * doc_h as usize * 4];
    draw_nodes(&mut out, nodes, doc_w, doc_h);
    out
}

fn draw_nodes(dst: &mut [u8], nodes: &[Node], doc_w: u32, doc_h: u32) {
    for node in nodes {
        match node {
            Node::Layer(layer) => draw_layer(dst, doc_w, doc_h, layer),
            Node::Group(group) => {
                if !group.visible || group.opacity <= 0.0 {
                    continue;
                }
                let mut temporaneo = vec![0u8; doc_w as usize * doc_h as usize * 4];
                draw_nodes(&mut temporaneo, &group.children, doc_w, doc_h);
                // opacità e maschera del gruppo scalano il risultato dei figli
                scale_alpha(
                    &mut temporaneo,
                    doc_w,
                    doc_h,
                    group.opacity,
                    group.mask.as_deref(),
                );
                blend_buffer(dst, &temporaneo, group.blend, doc_w, doc_h);
            }
        }
    }
}

/// Moltiplica l'alpha di un buffer per un fattore e una maschera (se c'è).
pub fn scale_alpha(buf: &mut [u8], doc_w: u32, doc_h: u32, opacity: f32, mask: Option<&Mask>) {
    if doc_w == 0 || doc_h == 0 {
        return;
    }
    let opacity = opacity.clamp(0.0, 1.0);
    for y in 0..doc_h {
        for x in 0..doc_w {
            let i = ((y as usize * doc_w as usize) + x as usize) * 4;
            let mut a = buf[i + 3] as f32 / 255.0 * opacity;
            if let Some(mask) = mask {
                let u = (x as f32 + 0.5) / doc_w as f32;
                let v = (y as f32 + 0.5) / doc_h as f32;
                a *= mask.sample(u, v);
            }
            buf[i + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// Fonde un buffer sopra un altro con la regola di blend data.
pub fn blend_buffer(dst: &mut [u8], src: &[u8], mode: BlendMode, doc_w: u32, doc_h: u32) {
    let pixel = doc_w as usize * doc_h as usize;
    for i in 0..pixel {
        let at = i * 4;
        if src[at + 3] == 0 {
            continue;
        }
        blend_px(&mut dst[at..at + 4], &src[at..at + 4], mode, 1.0);
    }
}

/// Disegna un layer sopra un buffer già esistente (in-place).
pub fn draw_layer(dst: &mut [u8], doc_w: u32, doc_h: u32, layer: &Layer) {
    if doc_w == 0 || doc_h == 0 || !layer.is_drawable() {
        return;
    }
    let Some(inverse) = layer.transform.doc_to_layer() else {
        return;
    };

    // Riquadro d'intersezione: si trasformano i 4 angoli del layer e si ritaglia
    // sul documento, così i layer grandi ma fuori campo non costano nulla.
    let (w, h) = (layer.width as f32, layer.height as f32);
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (px, py) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
        let (x, y) = layer.transform.map_point(px, py);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
        return;
    }

    let x0 = min_x.floor().clamp(0.0, doc_w as f32) as u32;
    let y0 = min_y.floor().clamp(0.0, doc_h as f32) as u32;
    let x1 = max_x.ceil().clamp(0.0, doc_w as f32) as u32;
    let y1 = max_y.ceil().clamp(0.0, doc_h as f32) as u32;

    for dy in y0..y1 {
        for dx in x0..x1 {
            // Centro del pixel: campionamento nearest, sufficiente come riferimento.
            let (lx, ly) = inverse.map_point(dx as f32 + 0.5, dy as f32 + 0.5);
            if !(lx.is_finite() && ly.is_finite()) || lx < 0.0 || ly < 0.0 {
                continue;
            }
            let (ix, iy) = (lx as u32, ly as u32);
            if ix >= layer.width || iy >= layer.height {
                continue;
            }
            let si = ((iy as usize * layer.width as usize) + ix as usize) * 4;
            if layer.pixels[si + 3] == 0 {
                continue; // pixel sorgente trasparente: nessun effetto
            }

            // La maschera è campionata con le UV del layer: dimensioni diverse
            // vengono adattate (nearest + clamp), come fa il sampler della GPU.
            let opacity = match layer.mask.as_deref() {
                Some(mask) => {
                    let u = lx / layer.width as f32;
                    let v = ly / layer.height as f32;
                    layer.opacity * mask.sample(u, v)
                }
                None => layer.opacity,
            };
            if opacity <= 0.0 {
                continue;
            }

            let di = ((dy as usize * doc_w as usize) + dx as usize) * 4;
            crate::blend::blend_px(
                &mut dst[di..di + 4],
                &layer.pixels[si..si + 4],
                layer.blend,
                opacity,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend::BlendMode;
    use crate::transform::Transform;

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y as usize * w as usize) + x as usize) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn un_solo_layer_opaco_riempie_il_documento() {
        let mut doc = Document::new(4, 4);
        doc.add_layer(Layer::filled("rosso", 4, 4, [255, 0, 0, 255]));
        let out = doc.composite();
        assert_eq!(px(&out, 4, 0, 0), [255, 0, 0, 255]);
        assert_eq!(px(&out, 4, 3, 3), [255, 0, 0, 255]);
    }

    #[test]
    fn il_documento_vuoto_e_trasparente() {
        let doc = Document::new(2, 2);
        assert_eq!(doc.composite(), vec![0u8; 2 * 2 * 4]);
    }

    #[test]
    fn nero_al_50_percento_su_bianco_da_grigio_medio() {
        let mut doc = Document::new(2, 2);
        doc.add_layer(Layer::filled("bianco", 2, 2, [255, 255, 255, 255]));
        let mut nero = Layer::filled("nero", 2, 2, [0, 0, 0, 255]);
        nero.opacity = 0.5;
        doc.add_layer(nero);
        let out = doc.composite();
        assert_eq!(px(&out, 2, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn un_layer_spostato_disegna_solo_dove_cade() {
        let mut doc = Document::new(8, 8);
        let mut l = Layer::filled("rosso", 2, 2, [255, 0, 0, 255]);
        l.transform = Transform::at(4.0, 4.0);
        doc.add_layer(l);
        let out = doc.composite();
        assert_eq!(px(&out, 8, 4, 4), [255, 0, 0, 255]);
        assert_eq!(px(&out, 8, 5, 5), [255, 0, 0, 255]);
        assert_eq!(px(&out, 8, 0, 0), [0, 0, 0, 0], "fuori dal layer");
        assert_eq!(px(&out, 8, 6, 6), [0, 0, 0, 0], "oltre il bordo");
    }

    #[test]
    fn un_layer_nascosto_non_si_vede() {
        let mut doc = Document::new(2, 2);
        let mut l = Layer::filled("rosso", 2, 2, [255, 0, 0, 255]);
        l.visible = false;
        doc.add_layer(l);
        assert_eq!(px(&doc.composite(), 2, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn opacita_zero_non_cambia_nulla() {
        let mut doc = Document::new(2, 2);
        doc.add_layer(Layer::filled("bianco", 2, 2, [255, 255, 255, 255]));
        let mut l = Layer::filled("nero", 2, 2, [0, 0, 0, 255]);
        l.opacity = 0.0;
        doc.add_layer(l);
        assert_eq!(px(&doc.composite(), 2, 0, 0), [255, 255, 255, 255]);
    }

    #[test]
    fn il_blend_mode_arriva_fino_al_buffer() {
        let mut doc = Document::new(2, 2);
        doc.add_layer(Layer::filled("bianco", 2, 2, [255, 255, 255, 255]));
        let mut l = Layer::filled("grigio", 2, 2, [128, 128, 128, 255]);
        l.blend = BlendMode::Multiply;
        doc.add_layer(l);
        // multiply(1.0, ~0.5) = ~0.5 -> resta grigio medio
        let out = doc.composite();
        assert_eq!(px(&out, 2, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn un_layer_scalato_copre_l_area_scalata() {
        let mut doc = Document::new(8, 8);
        let mut l = Layer::filled("rosso", 2, 2, [255, 0, 0, 255]);
        l.transform.scale_x = 2.0;
        l.transform.scale_y = 2.0;
        doc.add_layer(l);
        let out = doc.composite();
        assert_eq!(px(&out, 8, 3, 3), [255, 0, 0, 255], "dentro i 4x4 scalati");
        assert_eq!(px(&out, 8, 4, 4), [0, 0, 0, 0], "fuori");
    }

    #[test]
    fn scala_zero_non_disegna_e_non_va_in_panico() {
        let mut doc = Document::new(4, 4);
        let mut l = Layer::filled("rosso", 2, 2, [255, 0, 0, 255]);
        l.transform.scale_x = 0.0;
        doc.add_layer(l);
        assert_eq!(px(&doc.composite(), 4, 0, 0), [0, 0, 0, 0]);
    }

    // ---------------------------------------------------------------
    // Maschere: l'alpha del layer moltiplicata per il canale della maschera
    // ---------------------------------------------------------------

    use crate::mask::{Mask, MaskSource};
    use std::sync::Arc;

    fn doc_con_maschera(mask: Mask) -> Document {
        let mut doc = Document::new(4, 4);
        let mut l = Layer::filled("rosso", 4, 4, [255, 0, 0, 255]);
        l.mask = Some(Arc::new(mask));
        doc.add_layer(l);
        doc
    }

    #[test]
    fn mask_full_lascia_passare_tutto() {
        let senza = {
            let mut doc = Document::new(4, 4);
            doc.add_layer(Layer::filled("rosso", 4, 4, [255, 0, 0, 255]));
            doc.composite()
        };
        let con = doc_con_maschera(Mask::full(4, 4)).composite();
        assert_eq!(
            con, senza,
            "una maschera piena deve essere trasparente al risultato"
        );
    }

    #[test]
    fn mask_empty_nasconde_tutto() {
        let out = doc_con_maschera(Mask::empty(4, 4)).composite();
        assert!(
            out.as_chunks::<4>().0.iter().all(|p| p[3] == 0),
            "tutto nascosto"
        );
    }

    #[test]
    fn mask_meta_riduce_alpha_della_meta() {
        let mask = Mask::new(vec![128; 16], 4, 4, MaskSource::Procedural);
        let out = doc_con_maschera(mask).composite();
        let alpha = px(&out, 4, 1, 1)[3];
        assert!(
            (alpha as i32 - 128).abs() <= 2,
            "alpha atteso ~128, trovato {alpha}"
        );
    }

    #[test]
    fn mask_inverted_inverte() {
        let mut mask = Mask::new(vec![200; 16], 4, 4, MaskSource::Procedural);
        mask.set_inverted(true);
        let out = doc_con_maschera(mask).composite();
        let alpha = px(&out, 4, 1, 1)[3] as i32;
        assert!(
            (alpha - 55).abs() <= 3,
            "200 invertito deve dare ~55, trovato {alpha}"
        );
    }

    #[test]
    fn la_maschera_di_dimensioni_diverse_viene_adattata() {
        // maschera 2x2 su layer 4x4: ogni valore della maschera copre 2x2 pixel
        let mask = Mask::new(vec![0, 255, 255, 0], 2, 2, MaskSource::Procedural);
        let doc = doc_con_maschera(mask);
        let out = doc.composite();
        assert_eq!(px(&out, 4, 0, 0)[3], 0, "quadrante in alto a sinistra");
        assert_eq!(px(&out, 4, 3, 0)[3], 255, "alto a destra");
        assert_eq!(px(&out, 4, 0, 3)[3], 255, "basso a sinistra");
        assert_eq!(px(&out, 4, 3, 3)[3], 0, "basso a destra");
    }

    #[test]
    fn una_maschera_da_tessera_rivela_solo_dove_il_contenuto_e_opaco() {
        use crate::image::PlacedImage;
        // tessera 4x4 con una sola colonna opaca (x = 1)
        let mut rgba = vec![0u8; 4 * 4 * 4];
        for y in 0..4 {
            let i = ((y * 4 + 1) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
        let tile = PlacedImage {
            rgba,
            width: 4,
            height: 4,
            offset_x: 0,
            offset_y: 0,
            canvas_w: 4,
            canvas_h: 4,
        };
        let mask = Mask::from_tile(&tile);
        let out = doc_con_maschera(mask).composite();
        assert_eq!(px(&out, 4, 1, 0)[3], 255, "colonna della tessera visibile");
        assert_eq!(px(&out, 4, 0, 0)[3], 0, "il resto è nascosto");
        assert_eq!(px(&out, 4, 2, 0)[3], 0);
    }
}
