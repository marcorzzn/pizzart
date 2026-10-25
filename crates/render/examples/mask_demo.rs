//! Maschera come canale alpha: un testo rivela un gradiente.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-render --example mask_demo
//! ```
//!
//! Produce `mask_demo.png`. La cosa da notare: il layer del gradiente non è
//! stato ritagliato né distrutto. Ha una maschera — il canale alpha del testo —
//! e la composizione fa il resto. Cambiare la maschera cambia il risultato,
//! senza toccare i pixel del gradiente.

use pizzart_core::{Document, Layer, Mask, MaskSource};
use pizzart_render::HeadlessRenderer;
use pizzart_text::{FontRegistry, Text, TextStyle};

const W: u32 = 900;
const H: u32 = 520;

fn main() {
    let mut registry = FontRegistry::new();
    let family = registro(&mut registry);

    let renderer = match HeadlessRenderer::new() {
        Some(r) => r,
        None => {
            eprintln!("nessuna GPU raggiungibile");
            std::process::exit(1);
        }
    };
    println!("GPU: {} — famiglia: {family}", renderer.adapter_name());

    // La maschera: il canale alpha del testo.
    let titolo = Text::new("PIZZART", TextStyle::new(150.0).family(family.clone())).at(30.0, 120.0);
    let tile = titolo.render(&mut registry);
    println!(
        "testo -> tessera {}x{} a ({}, {})",
        tile.width, tile.height, tile.offset_x, tile.offset_y
    );

    let mascherata = Mask::from_tile_placed(&tile, W, H);
    println!(
        "maschera: {}x{}, {} KB, sorgente {:?}",
        mascherata.width,
        mascherata.height,
        mascherata.byte_len() / 1024,
        mascherata.source
    );

    let mut doc = Document::new(W, H);

    // 1. Fondo bianco: è quello che si vede dove la maschera è 0.
    doc.add_layer(Layer::filled("fondo", W, H, [252, 252, 250, 255]));

    // 2. Gradiente con la maschera: le lettere sono gradiente, il resto no.
    let mut gradiente = Layer::new("gradiente", W, H, gradient(W, H));
    gradiente.mask = Some(std::sync::Arc::new(mascherata));
    doc.add_layer(gradiente);

    // 3. Una riga di testo normale sopra, per mostrare che i due mondi convivono:
    //    qui il testo è *contenuto*, non maschera.
    let sottotitolo = Text::new(
        "lo stesso testo, usato come canale alpha",
        TextStyle::new(26.0)
            .family(family.clone())
            .color([90, 96, 110, 255]),
    )
    .at(34.0, 420.0);
    let tile_sotto = sottotitolo.render(&mut registry);

    let img = {
        let mut tessere = Vec::new();
        if !tile_sotto.is_empty() {
            tessere.push(
                renderer
                    .upload_tile(&tile_sotto, pizzart_core::BlendMode::Normal, 1.0)
                    .expect("upload"),
            );
        }
        renderer
            .render_document_with_tiles(&doc, &tessere)
            .expect("render")
    };

    image::save_buffer(
        "mask_demo.png",
        &img.rgba,
        img.width,
        img.height,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto mask_demo.png ({}x{})", img.width, img.height);
    let _ = MaskSource::Selection;
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

/// Gradiente diagonale acceso: serve qualcosa che si veda *dentro* le lettere.
fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let t = y as f32 / (h - 1) as f32;
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let i = ((y * w + x) * 4) as usize;
            px[i] = (240.0 - 120.0 * u) as u8;
            px[i + 1] = (60.0 + 120.0 * t) as u8;
            px[i + 2] = (90.0 + 140.0 * (1.0 - t)) as u8;
            px[i + 3] = 255;
        }
    }
    px
}
