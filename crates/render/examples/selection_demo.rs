//! Selezione → maschera → modifica locale.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-render --example selection_demo
//! ```
//!
//! Produce `selection_demo.png`: tre pannelli.
//!
//! 1. il gradiente originale;
//! 2. gradiente + luminosità su una **selezione rettangolare**;
//! 3. gradiente + saturazione su una **selezione ellittica**.
//!
//! Il punto: l'effetto è calcolato su tutta la tessera, ma la maschera decide
//! dove si vede. È la differenza tra un editor di immagini e un photo editor.

use std::sync::Arc;

use pizzart_core::{Document, EllipseSelection, Layer, Mask, PlacedImage, RectSelection};
use pizzart_render::{Effect, HeadlessRenderer};

const PANEL: u32 = 300;
const GAP: u32 = 10;

fn main() {
    let renderer = match HeadlessRenderer::new() {
        Some(r) => r,
        None => {
            eprintln!("nessuna GPU raggiungibile");
            std::process::exit(1);
        }
    };
    println!("GPU: {}", renderer.adapter_name());

    let gradiente = PlacedImage {
        rgba: gradient(PANEL, PANEL),
        width: PANEL,
        height: PANEL,
        offset_x: 0,
        offset_y: 0,
        canvas_w: PANEL,
        canvas_h: PANEL,
    };

    // --- pannello 1: originale -------------------------------------------
    let originale = renderer
        .render(&doc_con(&gradiente.rgba, None))
        .expect("render originale");

    // --- pannello 2: luminosità rivelata da un rettangolo -----------------
    let piu_chiaro = renderer
        .apply_effects(&gradiente, &[Effect::brightness(0.3)])
        .expect("brightness");
    let rect = RectSelection {
        x: 40.0,
        y: 40.0,
        w: 220.0,
        h: 150.0,
    }
    .to_mask(PANEL, PANEL);
    println!(
        "rettangolo: maschera {}x{}, {} pixel coperti",
        rect.width,
        rect.height,
        rect.alpha.iter().filter(|a| **a > 0).count()
    );
    let pannello_rect = renderer
        .render(&doc_con(&gradiente.rgba, Some((&piu_chiaro.rgba, rect))))
        .expect("render rettangolo");

    // --- pannello 3: desaturazione rivelata da un'ellisse -----------------
    let desaturato = renderer
        .apply_effects(&gradiente, &[Effect::saturate(-0.5)])
        .expect("saturate");
    let ellisse =
        EllipseSelection::circle(PANEL as f32 / 2.0, PANEL as f32 / 2.0, PANEL as f32 * 0.34)
            .to_mask(PANEL, PANEL);
    println!(
        "ellisse: maschera {}x{}, {} pixel coperti",
        ellisse.width,
        ellisse.height,
        ellisse.alpha.iter().filter(|a| **a > 0).count()
    );
    let pannello_ellisse = renderer
        .render(&doc_con(&gradiente.rgba, Some((&desaturato.rgba, ellisse))))
        .expect("render ellisse");

    // --- canvas a tre pannelli -------------------------------------------
    let pannelli = 3;
    let larghezza = PANEL * pannelli + GAP * (pannelli + 1);
    let altezza = PANEL + GAP * 2;
    let mut tela = vec![0u8; (larghezza * altezza * 4) as usize];
    for px in tela.as_chunks_mut::<4>().0.iter_mut() {
        px.copy_from_slice(&[22, 24, 30, 255]);
    }
    for (i, pannello) in [originale, pannello_rect, pannello_ellisse]
        .into_iter()
        .enumerate()
    {
        let x = GAP as i32 + i as i32 * (PANEL as i32 + GAP as i32);
        blit(
            &pannello.rgba,
            PANEL,
            PANEL,
            &mut tela,
            larghezza,
            altezza,
            x,
            GAP as i32,
        );
    }

    image::save_buffer(
        "selection_demo.png",
        &tela,
        larghezza,
        altezza,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto selection_demo.png ({larghezza}x{altezza})");
}

/// Documento: gradiente sotto e, se richiesto, una copia con l'effetto
/// applicato **rivelata dalla maschera**.
fn doc_con(base: &[u8], sopra: Option<(&[u8], Mask)>) -> Document {
    let mut doc = Document::new(PANEL, PANEL);
    doc.add_layer(Layer::new("gradiente", PANEL, PANEL, base.to_vec()));

    if let Some((pixels, mask)) = sopra {
        let mut effetto = Layer::new("effetto", PANEL, PANEL, pixels.to_vec());
        effetto.mask = Some(Arc::new(mask));
        doc.add_layer(effetto);
    }
    doc
}

#[allow(clippy::too_many_arguments)] // helper di disegno in un esempio: 8 argomenti sono il minimo per essere esplicito
fn blit(src: &[u8], sw: u32, sh: u32, dst: &mut [u8], dw: u32, dh: u32, ox: i32, oy: i32) {
    for y in 0..sh {
        let dy = oy + y as i32;
        if dy < 0 || dy >= dh as i32 {
            continue;
        }
        for x in 0..sw {
            let dx = ox + x as i32;
            if dx < 0 || dx >= dw as i32 {
                continue;
            }
            let si = ((y * sw + x) * 4) as usize;
            let di = ((dy as u32 * dw + dx as u32) * 4) as usize;
            dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
}

fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let t = y as f32 / (h - 1) as f32;
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let i = ((y * w + x) * 4) as usize;
            px[i] = (40.0 + 200.0 * u) as u8;
            px[i + 1] = (70.0 + 120.0 * (1.0 - t)) as u8;
            px[i + 2] = (170.0 - 80.0 * u) as u8;
            px[i + 3] = 255;
        }
    }
    px
}
