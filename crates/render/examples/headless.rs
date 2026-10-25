//! Esempio end-to-end: costruisce un documento, lo compone su CPU e su GPU,
//! confronta i due risultati e salva un'immagine.
//!
//! ```bash
//! cargo run --offline -p pizzart-render --example headless
//! ```
//!
//! Il file scritto è `output.ppm` (formato non compresso, nessuna dipendenza):
//! si apre con GIMP, IrfanView, o si converte in PNG con Pillow.

use std::time::Instant;

use pizzart_core::{BlendMode, Document, Layer, Transform};
use pizzart_render::HeadlessRenderer;

const W: u32 = 800;
const H: u32 = 500;

fn main() {
    let mut doc = Document::new(W, H);

    // 1. Sfondo: sfumatura verticale.
    doc.add_layer(Layer::new("sfondo", W, H, vertical_gradient(W, H)));

    // 2. Un "sole" morbido, in Screen: schiarisce senza bruciare i colori.
    let mut sole = Layer::new(
        "sole",
        320,
        320,
        radial(W, 320, 160.0, [255, 214, 120], 1.0),
    );
    sole.transform = Transform::at(80.0, 40.0);
    sole.blend = BlendMode::Screen;
    doc.add_layer(sole);

    // 3. Un disco caldo in Multiply, semitrasparente, spostato.
    let mut disco = Layer::new(
        "disco",
        300,
        300,
        radial(W, 300, 150.0, [180, 60, 200], 1.0),
    );
    disco.transform = Transform::at(330.0, 120.0);
    disco.blend = BlendMode::Multiply;
    disco.opacity = 0.85;
    doc.add_layer(disco);

    // 4. Tre barre diagonali, ciascuna con un blend diverso.
    for (i, (mode, color)) in [
        (BlendMode::Overlay, [255, 255, 255, 190]),
        (BlendMode::Difference, [90, 255, 220, 170]),
        (BlendMode::Exclusion, [255, 90, 140, 150]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut barra = Layer::filled(format!("barra {i}"), 160, 160, color);
        barra.transform = Transform::at(60.0 + 190.0 * i as f32, 300.0);
        barra.transform.rotation = 0.18;
        barra.blend = mode;
        doc.add_layer(barra);
    }

    // 5. Un quadretto scalato in HardLight, per mostrare la scala.
    let mut quad = Layer::new(
        "quadro",
        120,
        120,
        radial(W, 120, 60.0, [255, 255, 255], 0.6),
    );
    quad.transform = Transform::at(620.0, 300.0);
    quad.transform.scale_x = 1.4;
    quad.transform.scale_y = 0.8;
    quad.blend = BlendMode::HardLight;
    doc.add_layer(quad);

    println!(
        "documento {W}x{H}, {} layer ({})",
        doc.layer_count(),
        doc.history.labels().len()
    );

    let renderer = match HeadlessRenderer::new() {
        Some(r) => r,
        None => {
            eprintln!("nessuna GPU raggiungibile: esco senza scrivere l'immagine");
            std::process::exit(1);
        }
    };
    println!("GPU: {}", renderer.adapter_name());

    // Riferimento su CPU.
    let t0 = Instant::now();
    let cpu = doc.composite();
    let cpu_ms = t0.elapsed().as_secs_f64() * 1000.0;

    // Compositing su GPU.
    let t1 = Instant::now();
    let gpu = renderer.render(&doc).expect("render GPU");
    let gpu_ms = t1.elapsed().as_secs_f64() * 1000.0;

    // Confronto.
    let mut max_diff = 0i32;
    let mut somma = 0i64;
    for (a, b) in cpu.iter().zip(gpu.rgba.iter()) {
        let d = (*a as i32 - *b as i32).abs();
        max_diff = max_diff.max(d);
        somma += d as i64;
    }
    let media = somma as f64 / cpu.len() as f64;

    println!("CPU composite : {cpu_ms:8.2} ms");
    println!("GPU render    : {gpu_ms:8.2} ms  (upload + pass + readback)");
    println!("differenza CPU/GPU: max {max_diff}/255, media {media:.3}/255");

    write_ppm("output.ppm", gpu.width, gpu.height, &gpu.rgba).expect("scrittura PPM");
    println!("scritto output.ppm ({}x{})", gpu.width, gpu.height);
}

/// Sfumatura verticale, in sRGB diretto.
fn vertical_gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let t = y as f32 / (h - 1).max(1) as f32;
        let r = (24.0 + 30.0 * t) as u8;
        let g = (32.0 + 60.0 * t) as u8;
        let b = (72.0 + 120.0 * t) as u8;
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            px[i] = r;
            px[i + 1] = g;
            px[i + 2] = b;
            px[i + 3] = 255;
        }
    }
    px
}

/// Disco con bordo morbido: l'alpha sfuma negli ultimi 2 pixel del raggio.
fn radial(w: u32, size: u32, radius: f32, rgb: [u8; 3], intensita: f32) -> Vec<u8> {
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
    let _ = w;
    px
}

/// Scrive un PPM (P6) a partire da RGBA8: il canale alpha viene ignorato,
/// e i pixel trasparenti diventano neri.
fn write_ppm(path: &str, w: u32, h: u32, rgba: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = Vec::with_capacity((w * h * 3 + 32) as usize);
    out.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for chunk in rgba.chunks_exact(4) {
        out.extend_from_slice(&chunk[0..3]);
    }
    std::fs::File::create(path)?.write_all(&out)
}
