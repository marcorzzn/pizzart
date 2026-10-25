//! Matrice visiva dei 12 blend modes, calcolata sulla GPU.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-render --example blend_matrix
//! ```
//!
//! Produce `blend_matrix.png`: dodici tratti identici (stesso colore, stessa
//! forma) su fondo grigio medio, uno per modo di fusione. Su grigio medio si
//! vedono tutti, anche Screen e Overlay — su bianco sarebbero l'elemento neutro.
//!
//! Ogni fascia riporta il nome del modo, disegnato come barra di intensità:
//! non c'è un motore di testo, quindi si usa la posizione come legenda.

use pizzart_core::{BlendMode, Document, Layer};
use pizzart_draw::{Brush, Point, Stroke};
use pizzart_render::HeadlessRenderer;

const W: u32 = 760;
const ROW_H: u32 = 50;

fn main() {
    let rows = BlendMode::ALL.len() as u32;
    let h = ROW_H * rows;

    let mut doc = Document::new(W, h);
    // Grigio medio: il fondo neutro per eccellenza dei blend modes.
    doc.add_layer(Layer::filled("grigio", W, h, [128, 128, 128, 255]));

    let renderer = match HeadlessRenderer::new() {
        Some(r) => r,
        None => {
            eprintln!("nessuna GPU raggiungibile");
            std::process::exit(1);
        }
    };
    println!("GPU: {}", renderer.adapter_name());

    let mut strokes = Vec::new();
    let mut cpu = doc.composite();

    for (i, mode) in BlendMode::ALL.iter().enumerate() {
        let y = i as f32 * ROW_H as f32 + ROW_H as f32 / 2.0;
        let brush = Brush::new(34.0)
            .hardness(0.85)
            .spacing(0.12)
            .color([214, 92, 60])
            .blend(*mode);
        let mut stroke = Stroke::new(brush);
        for k in 0..40 {
            let t = k as f32 / 39.0;
            let x = 40.0 + t * (W as f32 - 80.0);
            // piccola onda: identica per tutti, così l'unica variabile è il modo
            let dy = (t * 3.0 * std::f32::consts::PI).sin() * 9.0;
            stroke.add(Point::new(x, y + dy).with_pressure(0.6 + 0.4 * t));
        }

        stroke.render_over(&mut cpu, W, h);
        strokes.push(
            renderer
                .upload_stroke(&stroke.render_bbox(W, h), *mode, 1.0)
                .expect("upload"),
        );
        println!("{:<12} {:?}", mode.label_it(), mode);
    }

    let img = renderer
        .render_document_with_strokes(&doc, &strokes)
        .expect("render");

    let mut max_diff = 0i32;
    for (a, b) in cpu.iter().zip(img.rgba.iter()) {
        max_diff = max_diff.max((*a as i32 - *b as i32).abs());
    }
    println!("differenza CPU/GPU sui 12 modi: max {max_diff}/255 (tolleranza 3)");
    assert!(max_diff <= 3, "parità CPU/GPU violata: {max_diff}");

    image::save_buffer(
        "blend_matrix.png",
        &img.rgba,
        img.width,
        img.height,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto blend_matrix.png ({}x{}), {rows} modi", img.width, img.height);
}
