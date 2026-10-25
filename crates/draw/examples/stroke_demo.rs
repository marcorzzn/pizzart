//! Dimostrazione del motore di disegno: sei tratti, uno per blend mode.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-draw --example stroke_demo
//! ```
//!
//! Produce `stroke_demo.png`: sei tratti su fondo bianco, con pressione,
//! durezza e spaziatura diverse, composti ognuno con un modo di fusione
//! differente.

use std::time::Instant;

use pizzart_core::BlendMode;
use pizzart_draw::{Brush, Point, Stroke};

const W: u32 = 960;
const H: u32 = 640;

fn main() {
    // Fondo grigio chiarissimo con una leggera sfumatura.
    //
    // Non bianco puro: Screen e Overlay su bianco sono l'elemento neutro
    // (screen(1, x) = 1, overlay(1, x) = 1), quindi i tratti sparirebbero per
    // definizione, non per un difetto. Un fondo a ~200/255 mostra tutti i modi.
    let mut canvas = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        let v = (247.0 - 55.0 * (y as f32 / H as f32)) as u8;
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            canvas[i] = v;
            canvas[i + 1] = v;
            canvas[i + 2] = (v as f32 * 0.99) as u8;
            canvas[i + 3] = 255;
        }
    }

    struct Caso {
        blend: BlendMode,
        color: [u8; 3],
        hardness: f32,
        spacing: f32,
        y: f32,
        pressione_decrescente: bool,
    }

    let casi = [
        Caso { blend: BlendMode::Normal, color: [24, 40, 110], hardness: 1.0, spacing: 0.10, y: 70.0, pressione_decrescente: false },
        Caso { blend: BlendMode::Multiply, color: [210, 30, 140], hardness: 0.15, spacing: 0.12, y: 165.0, pressione_decrescente: true },
        Caso { blend: BlendMode::Screen, color: [0, 150, 220], hardness: 0.50, spacing: 0.15, y: 260.0, pressione_decrescente: false },
        Caso { blend: BlendMode::Overlay, color: [235, 140, 30], hardness: 0.90, spacing: 0.10, y: 355.0, pressione_decrescente: true },
        Caso { blend: BlendMode::Difference, color: [20, 180, 90], hardness: 0.10, spacing: 0.20, y: 450.0, pressione_decrescente: false },
        Caso { blend: BlendMode::Exclusion, color: [140, 20, 40], hardness: 0.80, spacing: 0.60, y: 545.0, pressione_decrescente: true },
    ];

    let mut totale_ms = 0.0f64;
    let mut totale_timbri = 0usize;

    for (i, caso) in casi.iter().enumerate() {
        let brush = Brush::new(52.0)
            .hardness(caso.hardness)
            .spacing(caso.spacing)
            .color(caso.color)
            .blend(caso.blend)
            .stabilize(35.0);

        let mut stroke = Stroke::new(brush);

        // Percorso a onda: 120 punti, pressione da 0.1 a 1.0 (o viceversa).
        let n = 120;
        for k in 0..n {
            let t = k as f32 / (n - 1) as f32;
            let x = 70.0 + t * (W as f32 - 140.0);
            let y = caso.y + (t * 4.0 * std::f32::consts::PI).sin() * 22.0;
            let p = if caso.pressione_decrescente { 1.0 - t * 0.85 } else { 0.15 + t * 0.85 };
            stroke.add(Point::new(x, y).with_pressure(p).with_time(k as f64 * 8.0));
        }

        let timbri = stroke.stamps().len();
        let inizio = Instant::now();
        let toccati = stroke.render_over(&mut canvas, W, H);
        let ms = inizio.elapsed().as_secs_f64() * 1000.0;

        totale_ms += ms;
        totale_timbri += timbri;
        println!(
            "{}. {:<12} {:>5} timbri, {:>7} px, {ms:6.2} ms  (hardness {:.2}, spacing {:.2})",
            i + 1,
            format!("{:?}", caso.blend),
            timbri,
            toccati,
            caso.hardness,
            caso.spacing
        );
    }

    println!(
        "\ntotale: {totale_timbri} timbri, {totale_ms:.2} ms su {W}x{H} (CPU, single thread)"
    );

    image::save_buffer(
        "stroke_demo.png",
        &canvas,
        W,
        H,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto stroke_demo.png");
}
