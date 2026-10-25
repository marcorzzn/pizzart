//! Gruppi: due layer diventano una cosa sola, e l'appiattimento non cambia nulla.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-render --example groups_demo
//! ```
//!
//! Produce `groups_demo.png`, tre pannelli:
//!
//! 1. documento piatto: gradiente + cerchio semitrasparente;
//! 2. gli stessi due layer dentro un gruppo con blend **Multiply** e opacità 0.7;
//! 3. gli stessi due layer in un gruppo **neutro**, appiattito con `FlattenGroup`.
//!
//! (1) e (3) sono identici — l'appiattimento produce esattamente il composito —
//! mentre (2) è diverso, perché il blend del gruppo agisce su tutto il gruppo.

use pizzart_core::{BlendMode, Document, Group, Layer, Node};
use pizzart_render::HeadlessRenderer;

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

    // --- pannello 1: piatto --------------------------------------------------
    let piatto = renderer.render(&doc_piatto()).expect("render piatto");

    // --- pannello 2: gli stessi layer in un gruppo Multiply 0.7 --------------
    let con_gruppo = renderer.render(&doc_gruppo()).expect("render gruppo");

    // --- pannello 3: gruppo neutro, poi appiattito ---------------------------
    let mut doc = doc_neutro();
    // l'unico nodo della radice è il gruppo
    let gruppo = doc.root[0].id();
    doc.flatten_group(gruppo).expect("appiattimento");
    println!(
        "dopo flatten: {} layer, {} nodi in radice",
        doc.layer_count(),
        doc.root.len()
    );
    let appiattito = renderer.render(&doc).expect("render appiattito");

    let (diff_1_3, _) = max_diff(&piatto.rgba, &appiattito.rgba);
    let (diff_1_2, _) = max_diff(&piatto.rgba, &con_gruppo.rgba);
    println!("(1) vs (3) piatto/appiattito: differenza max {diff_1_3}/255");
    println!("(1) vs (2) piatto/gruppo:     differenza max {diff_1_2}/255");

    // --- canvas a tre pannelli ----------------------------------------------
    let pannelli = 3;
    let larghezza = PANEL * pannelli + GAP * (pannelli + 1);
    let altezza = PANEL + GAP * 2;
    let mut tela = vec![0u8; (larghezza * altezza * 4) as usize];
    for px in tela.as_chunks_mut::<4>().0.iter_mut() {
        px.copy_from_slice(&[22, 24, 30, 255]);
    }
    for (i, img) in [piatto, con_gruppo, appiattito].into_iter().enumerate() {
        let x = GAP as i32 + i as i32 * (PANEL as i32 + GAP as i32);
        blit(
            &img.rgba, PANEL, PANEL, &mut tela, larghezza, altezza, x, GAP as i32,
        );
    }

    image::save_buffer(
        "groups_demo.png",
        &tela,
        larghezza,
        altezza,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto groups_demo.png ({larghezza}x{altezza})");
}

fn due_layer() -> (Layer, Layer) {
    let gradiente = Layer::new("gradiente", PANEL, PANEL, gradient(PANEL, PANEL));
    let mut cerchio = Layer::new("cerchio", 220, 220, disc(220, [30, 30, 40], 0.55));
    cerchio.transform = pizzart_core::Transform::at(50.0, 50.0);
    (gradiente, cerchio)
}

fn doc_piatto() -> Document {
    let mut doc = Document::new(PANEL, PANEL);
    let (gradiente, cerchio) = due_layer();
    doc.add_layer(gradiente);
    doc.add_layer(cerchio);
    doc
}

fn doc_neutro() -> Document {
    let mut doc = Document::new(PANEL, PANEL);
    let (gradiente, cerchio) = due_layer();
    let mut gruppo = Group::new("gruppo");
    gruppo.add(Node::Layer(gradiente));
    gruppo.add(Node::Layer(cerchio));
    doc.add_node(Node::Group(gruppo));
    doc
}

fn doc_gruppo() -> Document {
    let mut doc = Document::new(PANEL, PANEL);
    let (gradiente, cerchio) = due_layer();
    let mut gruppo = Group::new("gruppo");
    gruppo.blend = BlendMode::Multiply;
    gruppo.opacity = 0.7;
    gruppo.add(Node::Layer(gradiente));
    gruppo.add(Node::Layer(cerchio));
    doc.add_node(Node::Group(gruppo));
    doc
}

fn max_diff(a: &[u8], b: &[u8]) -> (u8, usize) {
    let mut massimo = 0u8;
    let mut dove = 0usize;
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        let d = x.abs_diff(*y);
        if d > massimo {
            massimo = d;
            dove = i;
        }
    }
    (massimo, dove)
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
            px[i] = (250.0 - 60.0 * u) as u8;
            px[i + 1] = (140.0 + 90.0 * t) as u8;
            px[i + 2] = (60.0 + 170.0 * (1.0 - t)) as u8;
            px[i + 3] = 255;
        }
    }
    px
}

fn disc(size: u32, rgb: [u8; 3], intensita: f32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt();
            let edge = (c - d).clamp(0.0, 1.0);
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
