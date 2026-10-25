//! Integrazione completa: documento (sfondo + cerchio) e tre tratti disegnati
//! su GPU con tre blend modes diversi.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-render --example stroke_on_layer
//! ```
//!
//! Produce `stroke_on_layer.png`. È la prova visiva che la catena
//! `pizzart-draw` → `pizzart-render` funziona: il tratto nasce su CPU come
//! buffer RGBA8, diventa una texture GPU e si compone sopra i layer del
//! documento con il blend mode del pennello.

use pizzart_core::{BlendMode, Document, Layer, Transform};
use pizzart_draw::{Brush, Point, Stroke};
use pizzart_render::HeadlessRenderer;

const W: u32 = 900;
const H: u32 = 620;

fn main() {
    let mut doc = Document::new(W, H);

    // Sfondo: gradiente freddo.
    doc.add_layer(Layer::new("sfondo", W, H, gradient(W, H)));

    // Un cerchio caldo: è il "soggetto" su cui si vedono i blend modes.
    let mut cerchio = Layer::new("cerchio", 420, 420, disc(420, 190.0, [235, 176, 96], 0.9));
    cerchio.transform = Transform::at(150.0, 90.0);
    doc.add_layer(cerchio);

    let renderer = match HeadlessRenderer::new() {
        Some(r) => r,
        None => {
            eprintln!("nessuna GPU raggiungibile");
            std::process::exit(1);
        }
    };
    println!("GPU: {}", renderer.adapter_name());

    // Tre tratti che attraversano il cerchio, uno per blend mode.
    let casi = [
        (BlendMode::Normal, [26, 32, 64], 0.0),
        (BlendMode::Multiply, [200, 30, 120], 210.0),
        (BlendMode::Screen, [120, 230, 255], 420.0),
    ];

    let mut strokes = Vec::new();
    for (mode, color, offset) in casi {
        let brush = Brush::new(56.0)
            .hardness(0.35)
            .opacity(0.9)
            .spacing(0.12)
            .stabilize(40.0)
            .color(color)
            .blend(mode);

        let mut stroke = Stroke::new(brush);
        let n = 90;
        for k in 0..n {
            let t = k as f32 / (n - 1) as f32;
            let x = 70.0 + t * (W as f32 - 140.0);
            let y = 210.0 + offset * 0.35 + (t * 2.6 * std::f32::consts::PI).sin() * 95.0;
            let pressure = 0.35 + 0.65 * (t * std::f32::consts::PI).sin();
            stroke.add(Point::new(x, y).with_pressure(pressure));
        }

        strokes.push(
            renderer
                .upload_stroke(&stroke.render_bbox(W, H), mode, 1.0)
                .expect("upload dello stroke"),
        );
        println!("stroke {:?}: {} timbri", mode, stroke.stamps().len());
    }

    // Rendering: documenti + tratti, tutto su GPU.
    let img = renderer
        .render_document_with_strokes(&doc, &strokes)
        .expect("render");

    // Controllo di parità: stessa scena sul riferimento CPU.
    let mut cpu = doc.composite();
    for (mode, color, offset) in casi {
        let brush = Brush::new(56.0)
            .hardness(0.35)
            .opacity(0.9)
            .spacing(0.12)
            .stabilize(40.0)
            .color(color)
            .blend(mode);
        let mut stroke = Stroke::new(brush);
        let n = 90;
        for k in 0..n {
            let t = k as f32 / (n - 1) as f32;
            let x = 70.0 + t * (W as f32 - 140.0);
            let y = 210.0 + offset * 0.35 + (t * 2.6 * std::f32::consts::PI).sin() * 95.0;
            let pressure = 0.35 + 0.65 * (t * std::f32::consts::PI).sin();
            stroke.add(Point::new(x, y).with_pressure(pressure));
        }
        stroke.render_over(&mut cpu, W, H);
    }

    let mut max_diff = 0i32;
    for (a, b) in cpu.iter().zip(img.rgba.iter()) {
        max_diff = max_diff.max((*a as i32 - *b as i32).abs());
    }
    println!("differenza CPU/GPU sull'intera scena: max {max_diff}/255");

    image::save_buffer(
        "stroke_on_layer.png",
        &img.rgba,
        img.width,
        img.height,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto stroke_on_layer.png ({}x{})", img.width, img.height);
}

fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let t = y as f32 / (h - 1) as f32;
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let i = ((y * w + x) * 4) as usize;
            px[i] = (18.0 + 40.0 * u) as u8;
            px[i + 1] = (26.0 + 55.0 * t) as u8;
            px[i + 2] = (60.0 + 100.0 * (1.0 - t)) as u8;
            px[i + 3] = 255;
        }
    }
    px
}

fn disc(size: u32, radius: f32, rgb: [u8; 3], intensita: f32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt();
            let edge = (radius - d).clamp(0.0, 1.0);
            let a = (edge * 255.0 * intensita).round() as u8;
            let i = ((y * size + x) * 4) as usize;
            px[i] = rgb[0];
            px[i + 1] = rgb[1];
            px[i + 2] = rgb[2];
            px[i + 3] = a;
        }
    }
    px
}
