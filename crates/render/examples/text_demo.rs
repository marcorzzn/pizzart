//! Testo su GPU: registro font, layout, rasterizzazione CPU, composizione GPU.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-render --example text_demo
//! ```
//!
//! Produce `text_demo.png`. La cosa da notare: `pizzart-text` non sa nulla di
//! wgpu. Produce una `PlacedImage` — la stessa tessera di uno stroke — e il
//! renderer la compone senza una riga di codice nuovo.

use pizzart_core::{BlendMode, Document, Layer, Transform};
use pizzart_render::HeadlessRenderer;
use pizzart_text::{Align, FontRegistry, FontSource, Text, TextStyle};

const W: u32 = 900;
const H: u32 = 520;

fn main() {
    let mut doc = Document::new(W, H);
    doc.add_layer(Layer::new("sfondo", W, H, gradient(W, H)));

    let mut alone = Layer::new("alone", 420, 420, disc(420, 210.0, [226, 168, 92], 0.55));
    alone.transform = Transform::at(430.0, 60.0);
    doc.add_layer(alone);

    // Registro: usa un font di sistema se c'è, altrimenti uno dal disco.
    let mut registry = FontRegistry::new();
    let mut family = registro(&mut registry);
    println!("famiglia usata: {family}");

    let titolo = Text::new(
        "PIZZART",
        TextStyle::new(104.0)
            .family(family.clone())
            .color([255, 255, 255, 255]),
    )
    .at(64.0, 58.0);

    let sottotitolo = Text::new(
        "editor grafico · testo, effetti, disegno",
        TextStyle::new(26.0)
            .family(family.clone())
            .color([255, 214, 150, 255]),
    )
    .at(68.0, 186.0);

    let paragrafo = Text::new(
        "Il testo è una tessera come un tratto: si rasterizza su CPU, si ritaglia al suo ingombro e si compone su GPU. Nessuna pipeline nuova.",
        TextStyle::new(21.0)
            .family(family.clone())
            .color([236, 238, 250, 235])
            .line_height(1.45)
            .align(Align::Left),
    )
    .at(68.0, 250.0)
    .with_bounds(430.0, 0.0);

    let firma = Text::new(
        "— font da file, registro con validazione",
        TextStyle::new(17.0)
            .family(family.clone())
            .color([180, 190, 215, 220]),
    )
    .at(68.0, 430.0);

    let renderer = match HeadlessRenderer::new() {
        Some(r) => r,
        None => {
            eprintln!("nessuna GPU raggiungibile");
            std::process::exit(1);
        }
    };
    println!("GPU: {}", renderer.adapter_name());

    let mut tessere = Vec::new();
    for testo in [titolo, sottotitolo, paragrafo, firma] {
        let tile = testo.render(&mut registry);
        println!(
            "«{}» -> tessera {}x{} a ({}, {}) = {} KB",
            &testo.content[..testo.content.len().min(24)],
            tile.width,
            tile.height,
            tile.offset_x,
            tile.offset_y,
            tile.byte_len() / 1024
        );
        if tile.is_empty() {
            continue;
        }
        tessere.push(
            renderer
                .upload_tile(&tile, BlendMode::Normal, 1.0)
                .expect("upload della tessera di testo"),
        );
    }

    let img = renderer
        .render_document_with_tiles(&doc, &tessere)
        .expect("render");
    println!("composte {} tessere di testo", tessere.len());

    image::save_buffer(
        "text_demo.png",
        &img.rgba,
        img.width,
        img.height,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto text_demo.png ({}x{})", img.width, img.height);
    family.clear();
}

/// Sceglie una famiglia utilizzabile: prima dal sistema, poi da un file.
fn registro(registry: &mut FontRegistry) -> String {
    let families = registry.system_families();
    for preferita in [
        "Arial",
        "Segoe UI",
        "DejaVu Sans",
        "Helvetica",
        "Liberation Sans",
    ] {
        if families.iter().any(|f| f.eq_ignore_ascii_case(preferita)) {
            return preferita.to_string();
        }
    }
    if let Some(famiglia) = families.first() {
        return famiglia.clone();
    }
    // Nessun font di sistema: si carica un file dal disco.
    for path in [
        r"C:\Windows\Fonts\arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if std::path::Path::new(path).exists() {
            if let Ok(id) = registry.add_file(path) {
                return registry.get(id).unwrap().family.clone();
            }
        }
    }
    let _ = FontSource::Memory;
    "sans-serif".to_string()
}

fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let t = y as f32 / (h - 1) as f32;
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let i = ((y * w + x) * 4) as usize;
            px[i] = (18.0 + 40.0 * u) as u8;
            px[i + 1] = (22.0 + 30.0 * t) as u8;
            px[i + 2] = (58.0 + 90.0 * (1.0 - t)) as u8;
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
