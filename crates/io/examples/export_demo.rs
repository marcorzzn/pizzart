//! Export: il motore produce file che qualcuno può aprire.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-io --features webp --example export_demo
//! ```
//!
//! Compone su GPU un documento (gradiente + testo + cerchio mascherato) e lo
//! esporta in `export_demo.png`, `export_demo.jpg` e `export_demo.webp`,
//! stampando dimensioni e pesi. Il codice di codifica è lo stesso per tutti e
//! tre: cambia solo `ExportFormat`.

use pizzart_core::{BlendMode, Document, EllipseSelection, Group, Layer, Node};
use pizzart_io::{encode, ExportError, ExportFormat, ExportOptions};
use pizzart_render::HeadlessRenderer;
use pizzart_text::{FontRegistry, Text, TextStyle};

const W: u32 = 960;
const H: u32 = 600;

fn main() {
    let renderer = match HeadlessRenderer::new() {
        Some(r) => r,
        None => {
            eprintln!("nessuna GPU raggiungibile");
            std::process::exit(1);
        }
    };
    println!("GPU: {}", renderer.adapter_name());

    let mut doc = Document::new(W, H);
    doc.add_layer(Layer::new("gradiente", W, H, gradient(W, H)));

    // Un gruppo con un cerchio mascherato dentro: dà corpo alla scena e
    // dimostra che l'export non sa nulla di layer, gruppi o maschere.
    let mut gruppo = Group::new("scena");
    let mut alone = Layer::new("alone", 520, 520, disc(520, [255, 245, 210], 0.5));
    alone.transform = pizzart_core::Transform::at(300.0, 40.0);
    gruppo.add(Node::Layer(alone));
    doc.add_node(Node::Group(gruppo));

    let mut registry = FontRegistry::new();
    let family = registro(&mut registry);
    let titolo = Text::new(
        "PIZZART",
        TextStyle::new(128.0)
            .family(family.clone())
            .color([255, 255, 255, 255]),
    )
    .at(56.0, 96.0);
    let sottotitolo = Text::new(
        "export: PNG, JPEG, WebP",
        TextStyle::new(30.0)
            .family(family)
            .color([255, 240, 210, 245]),
    )
    .at(60.0, 250.0);

    let mut tessere = Vec::new();
    for testo in [titolo, sottotitolo] {
        let tile = testo.render(&mut registry);
        if !tile.is_empty() {
            tessere.push(
                renderer
                    .upload_tile(&tile, BlendMode::Normal, 1.0)
                    .expect("upload"),
            );
        }
    }

    // una selezione ellittica come maschera su un velo scuro: profondità
    let mut velo = Layer::new("velo", W, H, vec![20u8; (W * H * 4) as usize]);
    velo.opacity = 0.35;
    velo.mask = Some(std::sync::Arc::new(
        EllipseSelection::circle(W as f32 * 0.5, H as f32 * 0.62, 300.0).to_mask(W, H),
    ));
    doc.add_layer(velo);

    let img = renderer
        .render_document_with_tiles(&doc, &tessere)
        .expect("render");
    println!("composta: {}x{}", img.width, img.height);

    // --- PNG: senza perdita, con alpha -----------------------------------
    let opzioni = ExportOptions::default();
    match encode(&img, ExportFormat::Png, &opzioni) {
        Ok(bytes) => {
            std::fs::write("export_demo.png", &bytes).expect("scrittura png");
            println!("export_demo.png:   {} KB", bytes.len() / 1024);
        }
        Err(e) => println!("PNG fallito: {e}"),
    }

    // --- JPEG: con perdita, senza alpha (l'immagine è opaca) --------------
    match encode(&img, ExportFormat::Jpeg { quality: 90 }, &opzioni) {
        Ok(bytes) => {
            std::fs::write("export_demo.jpg", &bytes).expect("scrittura jpg");
            println!("export_demo.jpg:   {} KB (qualità 90)", bytes.len() / 1024);
        }
        Err(e) => println!("JPEG fallito: {e}"),
    }

    // --- WebP: senza perdita (l'encoder lossy non è in questa build) ------
    match encode(
        &img,
        ExportFormat::WebP {
            quality: 90,
            lossless: true,
        },
        &opzioni,
    ) {
        Ok(bytes) => {
            std::fs::write("export_demo.webp", &bytes).expect("scrittura webp");
            println!("export_demo.webp:  {} KB (lossless)", bytes.len() / 1024);
        }
        Err(ExportError::FeatureNotEnabled(what)) => {
            println!("WebP non disponibile in questa build: {what}");
        }
        Err(e) => println!("WebP fallito: {e}"),
    }

    // il confronto che conta: JPEG pesa meno ed è più veloce, PNG è esatto
    println!(
        "\nregola: PNG per la qualità e l'alpha, JPEG per il peso, WebP per entrambi\n\
         (webp qui è lossless: pesa come il PNG)"
    );
}

fn registro(registry: &mut FontRegistry) -> String {
    let families = registry.system_families();
    for preferita in ["Arial", "Segoe UI", "DejaVu Sans", "Helvetica"] {
        if families.iter().any(|f| f.eq_ignore_ascii_case(preferita)) {
            return preferita.to_string();
        }
    }
    if let Some(famiglia) = families.first() {
        return famiglia.clone();
    }
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
    "sans-serif".to_string()
}

fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let t = y as f32 / (h - 1) as f32;
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let i = ((y * w + x) * 4) as usize;
            px[i] = (30.0 + 190.0 * u) as u8;
            px[i + 1] = (40.0 + 120.0 * (1.0 - t)) as u8;
            px[i + 2] = (120.0 + 110.0 * (1.0 - u)) as u8;
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
