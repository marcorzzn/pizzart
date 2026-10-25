//! Roundtrip completo: documento con gruppi, maschera e testo, salvato e
//! ricaricato, verificato sul composito.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-project --example roundtrip
//! ```

use std::sync::Arc;

use pizzart_core::{BlendMode, Document, EllipseSelection, Group, Layer, Node, Transform};
use pizzart_project::{load, peek_manifest, save, Codec, SaveOptions};
use pizzart_text::{FontRegistry, Text, TextStyle};

fn main() {
    let mut doc = Document::new(640, 400);
    doc.add_layer(Layer::new("fondo", 640, 400, gradient(640, 400)));

    // un gruppo con un cerchio e un velo
    let mut gruppo = Group::new("scena");
    gruppo.opacity = 0.9;
    gruppo.blend = BlendMode::Screen;
    let mut cerchio = Layer::new("cerchio", 260, 260, disc(260, [240, 200, 120], 0.9));
    cerchio.transform = Transform::at(60.0, 70.0);
    gruppo.add(Node::Layer(cerchio));
    let mut velo = Layer::new("velo", 640, 400, vec![15u8; (640 * 400 * 4) as usize]);
    velo.opacity = 0.25;
    velo.mask = Some(Arc::new(
        EllipseSelection::circle(420.0, 220.0, 160.0).to_mask(640, 400),
    ));
    gruppo.add(Node::Layer(velo));
    doc.add_node(Node::Group(gruppo));

    // testo rasterizzato in un layer, come fa l'editor
    let mut registry = FontRegistry::new();
    let family = registro(&mut registry);
    let tile = Text::new(
        "PIZZART",
        TextStyle::new(96.0)
            .family(family)
            .color([255, 255, 255, 255]),
    )
    .at(40.0, 250.0)
    .render(&mut registry);
    if !tile.is_empty() {
        let mut testo = Layer::new("titolo", tile.width, tile.height, tile.rgba);
        testo.transform = Transform::at(tile.offset_x as f32, tile.offset_y as f32);
        doc.add_layer(testo);
    }

    let prima = doc.composite();
    println!(
        "documento: {}×{}, {} layer, {} nodi in radice",
        doc.width,
        doc.height,
        doc.layer_count(),
        doc.root.len()
    );

    let path = std::env::temp_dir().join("pizzart-roundtrip.pizzart");
    let opzioni = SaveOptions {
        codec: Codec::Zstd,
        ..SaveOptions::default()
    };
    let inizio = std::time::Instant::now();
    save(&doc, &path, &opzioni).expect("salvataggio");
    let tempo_save = inizio.elapsed();

    let dimensione = std::fs::metadata(&path).expect("metadati").len();
    println!(
        "salvato in {} ({} KB) in {tempo_save:?}",
        path.display(),
        dimensione / 1024
    );

    // il documento in memoria viene buttato: se qualcosa non è nel file, si perde
    drop(doc);

    let inizio = std::time::Instant::now();
    let ricaricato = load(&path).expect("caricamento");
    let tempo_load = inizio.elapsed();
    println!(
        "ricaricato: {}×{}, {} layer, {} nodi in radice, in {tempo_load:?}",
        ricaricato.width,
        ricaricato.height,
        ricaricato.layer_count(),
        ricaricato.root.len()
    );

    let dopo = ricaricato.composite();
    let diff = prima
        .iter()
        .zip(dopo.iter())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    println!(
        "composito: {} byte, differenza massima {diff}/255",
        prima.len()
    );
    assert_eq!(prima.len(), dopo.len(), "dimensioni del composito");
    if diff != 0 {
        eprintln!("ATTENZIONE: il composito non è identico");
        std::process::exit(1);
    }

    // ispezione del manifest senza caricare i pixel
    let manifest = peek_manifest(&path).expect("manifest");
    println!(
        "manifest: schema {}, {} nodi, {} asset",
        manifest.schema_version,
        manifest.nodes.len(),
        manifest.assets.len()
    );
    for nodo in &manifest.nodes {
        let tipo = match nodo.kind.as_str() {
            "group" => format!("gruppo ({} figli)", nodo.children.len()),
            _ => "layer".to_string(),
        };
        println!("  {:<10} {:<12} {}", nodo.name, tipo, nodo.uuid);
    }

    println!("\nroundtrip riuscito: il file si regge da solo");
}

fn registro(registry: &mut FontRegistry) -> String {
    let families = registry.system_families();
    for preferita in ["Arial", "Segoe UI", "DejaVu Sans", "Helvetica"] {
        if families.iter().any(|f| f.eq_ignore_ascii_case(preferita)) {
            return preferita.to_string();
        }
    }
    families.first().cloned().unwrap_or_else(|| "sans-serif".into())
}

fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let t = y as f32 / (h - 1) as f32;
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let i = ((y * w + x) * 4) as usize;
            px[i] = (40.0 + 150.0 * u) as u8;
            px[i + 1] = (30.0 + 90.0 * (1.0 - t)) as u8;
            px[i + 2] = (110.0 + 120.0 * (1.0 - u)) as u8;
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
