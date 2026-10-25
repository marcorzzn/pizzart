//! Tre pannelli: originale, luminosità, catena di tre effetti.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-render --example effects_demo
//! ```
//!
//! Produce `effects_demo.png`. La differenza deve essere visibile a occhio:
//! il pannello centrale è più chiaro, quello a destra è più contrastato e più
//! saturo dell'originale.

use pizzart_core::PlacedImage;
use pizzart_render::{cpu, Effect, HeadlessRenderer};

const PANEL: u32 = 240;
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

    let base = immagine_di_prova(PANEL, PANEL);

    // 1. originale
    let originale = base.clone();

    // 2. luminosità: tutto più chiaro
    let luminosa = renderer
        .apply_effects(&base, &[Effect::brightness(0.3)])
        .expect("brightness");
    println!("brightness(+0.3): ok");

    // 3. catena: più contrasto, più saturazione, luce misurata
    let catena = [
        Effect::brightness(0.12),
        Effect::contrast(0.35),
        Effect::saturate(0.6),
    ];
    let processata = renderer.apply_effects(&base, &catena).expect("catena");
    println!("catena di {} effetti: ok", catena.len());

    // 4. blur: kernel separabile, due pass
    let sfocata = renderer
        .apply_effects(&base, &[Effect::blur(6.0)])
        .expect("blur");
    println!("blur raggio 6: ok");

    // 5. nitidezza: kernel + combinazione con l'originale (3 pass)
    let nitida = renderer
        .apply_effects(&base, &[Effect::sharpen(2.0, 2.0)])
        .expect("sharpen");
    println!("sharpen: ok");

    // 6. bagliore: sfocato sopra soglia, aggiunto con Screen
    let luminosa_alone = renderer
        .apply_effects(&base, &[Effect::glow(0.25, 2.5, 10.0)])
        .expect("glow");
    println!("glow: ok");

    // Controllo di parità fra GPU e riferimento CPU, sulla catena completa.
    let riferimento = cpu::apply_effects(&base, &catena);
    let mut max_diff = 0i32;
    for (a, b) in riferimento.rgba.iter().zip(processata.rgba.iter()) {
        max_diff = max_diff.max((*a as i32 - *b as i32).abs());
    }
    println!("differenza CPU/GPU sulla catena: max {max_diff}/255");

    let riferimento_blur = cpu::apply_effects(&base, &[Effect::blur(6.0)]);
    let mut max_diff_blur = 0i32;
    for (a, b) in riferimento_blur.rgba.iter().zip(sfocata.rgba.iter()) {
        max_diff_blur = max_diff_blur.max((*a as i32 - *b as i32).abs());
    }
    println!("differenza CPU/GPU sul blur: max {max_diff_blur}/255");

    // Sei pannelli affiancati su fondo scuro.
    let pannelli = 6;
    let larghezza = PANEL * pannelli + GAP * (pannelli + 1);
    let altezza = PANEL + GAP * 2;
    let mut tela = vec![0u8; (larghezza * altezza * 4) as usize];
    for px in tela.chunks_exact_mut(4) {
        px.copy_from_slice(&[22, 24, 30, 255]);
    }

    for (i, pannello) in [
        originale,
        luminosa,
        processata,
        sfocata,
        nitida,
        luminosa_alone,
    ]
    .into_iter()
    .enumerate()
    {
        let x = GAP as i32 + i as i32 * (PANEL as i32 + GAP as i32);
        pannello
            .clone()
            .shifted(x, GAP as i32)
            .blit_into(&mut tela, larghezza, altezza);
    }

    image::save_buffer(
        "effects_demo.png",
        &tela,
        larghezza,
        altezza,
        image::ColorType::Rgba8,
    )
    .expect("scrittura PNG");
    println!("scritto effects_demo.png ({larghezza}x{altezza})");
}

/// Immagine di prova: gradiente, cerchi colorati e una banda di grigi.
fn immagine_di_prova(w: u32, h: u32) -> PlacedImage {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let centro = (w as f32 / 2.0, h as f32 / 2.0);

    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w - 1) as f32;
            let v = y as f32 / (h - 1) as f32;
            let i = ((y * w + x) * 4) as usize;

            // fondo: gradiente diagonale
            let mut c = [
                40.0 + 170.0 * u,
                30.0 + 150.0 * v,
                120.0 + 100.0 * (1.0 - u),
            ];

            // tre cerchi colorati, a bordo netto
            for (cx, cy, r, col) in [
                (90.0, 90.0, 52.0, [235.0, 70.0, 60.0]),
                (205.0, 120.0, 44.0, [60.0, 200.0, 120.0]),
                (140.0, 225.0, 58.0, [70.0, 110.0, 245.0]),
            ] {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                if d < r {
                    c = col;
                }
            }

            // scacchiera a quadretti: è la prova visiva del blur
            if x < 40 && y < 40 {
                let quadretto = ((x / 5) + (y / 5)) % 2;
                let v = if quadretto == 0 { 245.0 } else { 20.0 };
                c = [v, v, v];
            }

            // crocino giallo al centro
            let dx = x as f32 - centro.0;
            let dy = y as f32 - centro.1;
            if dx.abs() < 2.0 && dy.abs() < 12.0 || dy.abs() < 2.0 && dx.abs() < 12.0 {
                c = [255.0, 255.0, 0.0];
            }

            rgba[i] = c[0].clamp(0.0, 255.0) as u8;
            rgba[i + 1] = c[1].clamp(0.0, 255.0) as u8;
            rgba[i + 2] = c[2].clamp(0.0, 255.0) as u8;
            rgba[i + 3] = 255;
        }
    }

    PlacedImage {
        rgba,
        width: w,
        height: h,
        offset_x: 0,
        offset_y: 0,
        canvas_w: w,
        canvas_h: h,
    }
}
