//! Misura veloce: quanto costa codificare 2 MP in JPEG a seconda del contenuto.

use pizzart_io::{encode, ExportFormat, ExportOptions};
use pizzart_render::RenderedImage;

fn main() {
    // quanto costa solo la conversione RGB?
    let grande = immagine(1920, 1080, |x, _| [x as u8, 20, 30, 255]);
    let inizio = std::time::Instant::now();
    let rgb: Vec<u8> = grande
        .rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    println!(
        "conversione RGB pura: {:?} ({} byte)",
        inizio.elapsed(),
        rgb.len()
    );

    // scala del costo JPEG
    for lato in [(480u32, 270u32), (960, 540)] {
        let img = immagine(lato.0, lato.1, |_, _| [128u8, 128, 128, 255]);
        let mut min = std::time::Duration::from_secs(999);
        for _ in 0..3 {
            let inizio = std::time::Instant::now();
            let _ = encode(
                &img,
                ExportFormat::Jpeg { quality: 90 },
                &ExportOptions::default(),
            );
            min = min.min(inizio.elapsed());
        }
        let mp = lato.0 as f64 * lato.1 as f64 / 1e6;
        println!(
            "{}x{} ({mp:.2} MP): JPEG {min:?} -> {:.1} MP/s",
            lato.0,
            lato.1,
            mp / min.as_secs_f64()
        );
    }

    for (nome, img) in [
        (
            "uniforme",
            immagine(1920, 1080, |_, _| [128u8, 128, 128, 255]),
        ),
        (
            "gradiente liscio",
            immagine(1920, 1080, |x, y| {
                let t = (x * 255 / 1920) as u8;
                let u = (y * 255 / 1080) as u8;
                [t, u, ((t as u16 + u as u16) / 2) as u8, 255]
            }),
        ),
        (
            "rumore",
            immagine(1920, 1080, |x, y| {
                let n = ((x * 7 + y * 13) % 37) as u8;
                let t = (x * 255 / 1920) as u8;
                [t.wrapping_add(n), n, n.wrapping_mul(3), 255]
            }),
        ),
    ] {
        let opzioni = ExportOptions::default();
        let mut min_jpeg = std::time::Duration::from_secs(999);
        let mut min_png = std::time::Duration::from_secs(999);
        let mut dim_jpeg = 0;
        let mut dim_png = 0;
        for _ in 0..3 {
            let inizio = std::time::Instant::now();
            let b = encode(&img, ExportFormat::Jpeg { quality: 90 }, &opzioni).expect("jpeg");
            min_jpeg = min_jpeg.min(inizio.elapsed());
            dim_jpeg = b.len();

            let inizio = std::time::Instant::now();
            let b = encode(&img, ExportFormat::Png, &opzioni).expect("png");
            min_png = min_png.min(inizio.elapsed());
            dim_png = b.len();
        }
        println!(
            "{nome:<18} JPEG {min_jpeg:>8.1?} ({} KB)   PNG {min_png:>8.1?} ({} KB)",
            dim_jpeg / 1024,
            dim_png / 1024
        );
    }
}

fn immagine(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RenderedImage {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&f(x, y));
        }
    }
    RenderedImage {
        width: w,
        height: h,
        rgba,
    }
}
