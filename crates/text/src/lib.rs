//! # pizzart-text
//!
//! Testo: registro dei font, layout e rasterizzazione su CPU in una
//! `PlacedImage` — la stessa tessera che produce il motore di disegno.
//!
//! Il principio: **bytes → registro → handle → tessera**. Il registro non sa
//! da dove arrivano i byte (file, rete, memoria): chi carica fa l'I/O, la
//! libreria valida e rasterizza. È il caso più semplice e reale del pattern
//! "risorsa esterna", e serve a capire se il pattern regge prima di estenderlo.

#![forbid(unsafe_code)]

pub mod registry;

pub use registry::{
    FontError, FontId, FontInfo, FontRegistry, FontSource, MAX_FONTS, MAX_FONT_SIZE,
};

use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, SwashCache, SwashContent};
use pizzart_core::PlacedImage;

/// Allineamento orizzontale delle righe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Come appare il testo.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Famiglia da usare; vuota = quella di ripiego del sistema.
    pub family: String,
    pub size: f32,
    /// RGBA non premoltiplicato.
    pub color: [u8; 4],
    /// Interlinea come multiplo della dimensione (1.2 è un buon default).
    pub line_height: f32,
    pub align: Align,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: String::new(),
            size: 32.0,
            color: [0, 0, 0, 255],
            line_height: 1.2,
            align: Align::Left,
        }
    }
}

impl TextStyle {
    pub fn new(size: f32) -> Self {
        Self {
            size: size.max(1.0),
            ..Self::default()
        }
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.family = family.into();
        self
    }

    pub fn color(mut self, rgba: [u8; 4]) -> Self {
        self.color = rgba;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn line_height(mut self, multiplier: f32) -> Self {
        self.line_height = multiplier.max(0.5);
        self
    }
}

/// Un testo posizionato, con i limiti entro cui andare a capo.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub content: String,
    pub style: TextStyle,
    /// Angolo superiore sinistro nel documento.
    pub position: (f32, f32),
    /// Larghezza massima (per il wrapping) e altezza massima.
    /// Zero significa "nessun limite".
    pub bounds: (f32, f32),
}

impl Text {
    pub fn new(content: impl Into<String>, style: TextStyle) -> Self {
        Self {
            content: content.into(),
            style,
            position: (0.0, 0.0),
            bounds: (0.0, 0.0),
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.position = (x, y);
        self
    }

    pub fn with_bounds(mut self, width: f32, height: f32) -> Self {
        self.bounds = (width, height);
        self
    }

    /// Rasterizza il testo in una tessera ritagliata al suo ingombro.
    ///
    /// Stesso contratto di `Stroke::render_bbox`: la tessera porta con sé la
    /// posizione (`offset_x`, `offset_y`), quindi il renderer la compone come
    /// qualunque altro contenuto. Testo vuoto → tessera vuota.
    pub fn render(&self, registry: &mut FontRegistry) -> PlacedImage {
        let canvas_w = if self.bounds.0 > 0.0 {
            self.bounds.0.ceil() as u32
        } else {
            0
        };
        let canvas_h = if self.bounds.1 > 0.0 {
            self.bounds.1.ceil() as u32
        } else {
            0
        };

        if self.content.trim().is_empty() {
            return PlacedImage::empty(canvas_w, canvas_h);
        }

        let size = self.style.size.max(1.0);
        let line_px = size * self.style.line_height;
        let metrics = Metrics::new(size, line_px);
        let font_system = registry.system_mut();

        let mut buffer = Buffer::new(font_system, metrics);
        let width = if self.bounds.0 > 0.0 {
            Some(self.bounds.0)
        } else {
            None
        };
        let height = if self.bounds.1 > 0.0 {
            Some(self.bounds.1)
        } else {
            None
        };
        buffer.set_size(font_system, width, height);

        let attrs = if self.style.family.is_empty() {
            Attrs::new()
        } else {
            Attrs::new().family(Family::Name(&self.style.family))
        };
        buffer.set_text(font_system, &self.content, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(font_system, false);

        // Prima passata: rasterizza i glifi e misura l'ingombro reale.
        let mut cache = SwashCache::new();
        let mut draws: Vec<Draw> = Vec::new();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for run in buffer.layout_runs() {
            let line_width = run.line_w;
            let shift = match self.style.align {
                Align::Left => 0.0,
                Align::Center => (self.bounds.0 - line_width) * 0.5,
                Align::Right => self.bounds.0 - line_width,
            };
            let line_top = run.line_y as i32;

            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let Some(image) = cache.get_image_uncached(font_system, physical.cache_key) else {
                    continue;
                };
                let x = physical.x + shift as i32 + image.placement.left;
                let y = physical.y + line_top - image.placement.top;
                let (w, h) = (image.placement.width as i32, image.placement.height as i32);
                if w == 0 || h == 0 {
                    continue;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + w);
                max_y = max_y.max(y + h);
                draws.push(Draw { x, y, image });
            }
        }

        if draws.is_empty() || max_x <= min_x || max_y <= min_y {
            return PlacedImage::empty(canvas_w, canvas_h);
        }

        let (tile_w, tile_h) = ((max_x - min_x) as u32, (max_y - min_y) as u32);
        let pixels = tile_w as usize * tile_h as usize;

        // Prima la **copertura**, poi il colore: così il colore del testo è
        // esatto a ogni livello di antialiasing, senza perdite da
        // premoltiplicazione. Gli emoji (glifi già colorati) si compongono sopra.
        let mut mask = vec![0u8; pixels];
        let mut emoji = vec![0u8; pixels * 4];
        let color = self.style.color;

        for draw in &draws {
            let (w, h) = (draw.image.placement.width, draw.image.placement.height);
            let base_x = draw.x - min_x;
            let base_y = draw.y - min_y;
            match draw.image.content {
                SwashContent::Mask => {
                    for row in 0..h {
                        for col in 0..w {
                            let coverage = draw.image.data[(row * w + col) as usize];
                            if coverage == 0 {
                                continue;
                            }
                            let px = (base_x + col as i32) as usize;
                            let py = (base_y + row as i32) as usize;
                            let i = py * tile_w as usize + px;
                            mask[i] = mask[i].max(coverage);
                        }
                    }
                }
                SwashContent::Color => {
                    for row in 0..h {
                        for col in 0..w {
                            let si = ((row * w + col) * 4) as usize;
                            let px = (base_x + col as i32) as usize;
                            let py = (base_y + row as i32) as usize;
                            let i = ((py * tile_w as usize) + px) * 4;
                            over_straight(
                                &mut emoji[i..i + 4],
                                [
                                    draw.image.data[si],
                                    draw.image.data[si + 1],
                                    draw.image.data[si + 2],
                                    draw.image.data[si + 3],
                                ],
                            );
                        }
                    }
                }
                SwashContent::SubpixelMask => {
                    // Maschera subpixel: si usa il canale verde come copertura.
                    for row in 0..h {
                        for col in 0..w {
                            let si = ((row * w + col) * 3) as usize;
                            let coverage = draw.image.data[si + 1];
                            if coverage == 0 {
                                continue;
                            }
                            let px = (base_x + col as i32) as usize;
                            let py = (base_y + row as i32) as usize;
                            let i = py * tile_w as usize + px;
                            mask[i] = mask[i].max(coverage);
                        }
                    }
                }
            }
        }

        let mut rgba = vec![0u8; pixels * 4];
        for (i, copertura) in mask.iter().enumerate() {
            let coverage = *copertura as u32;
            let alpha = (coverage * color[3] as u32) / 255;
            let at = i * 4;
            if alpha > 0 {
                rgba[at] = color[0];
                rgba[at + 1] = color[1];
                rgba[at + 2] = color[2];
                rgba[at + 3] = alpha as u8;
            }
            // emoji sopra il colore del testo
            let ea = emoji[at + 3];
            if ea > 0 {
                over_straight(
                    &mut rgba[at..at + 4],
                    [emoji[at], emoji[at + 1], emoji[at + 2], ea],
                );
            }
        }

        PlacedImage {
            rgba,
            width: tile_w,
            height: tile_h,
            offset_x: self.position.0.floor() as i32 + min_x,
            offset_y: self.position.1.floor() as i32 + min_y,
            canvas_w: canvas_w.max(tile_w),
            canvas_h: canvas_h.max(tile_h),
        }
    }
}

/// Un glifo pronto per essere disegnato.
struct Draw {
    x: i32,
    y: i32,
    image: cosmic_text::SwashImage,
}

/// Composizione "source over" in alpha non premoltiplicato.
fn over_straight(dst: &mut [u8], src: [u8; 4]) {
    let sa = src[3] as u32;
    if sa == 0 {
        return;
    }
    let da = dst[3] as u32;
    let out_a = sa + da * (255 - sa) / 255;
    if out_a == 0 {
        return;
    }
    for c in 0..3 {
        let value = (src[c] as u32 * sa + dst[c] as u32 * da * (255 - sa) / 255) / out_a;
        dst[c] = value.min(255) as u8;
    }
    dst[3] = out_a.min(255) as u8;
}

/// Cerca un font di sistema utilizzabile per i test.
pub fn test_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\tahoma.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ];
    candidates.iter().find_map(|p| std::fs::read(p).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registro_con_font() -> Option<FontRegistry> {
        let bytes = test_font_bytes()?;
        let mut reg = FontRegistry::new();
        reg.add(bytes, FontSource::Memory).ok()?;
        Some(reg)
    }

    #[test]
    fn testo_vuoto_produce_tile_vuota() {
        let mut reg = registro_con_font().expect("font di sistema");
        let text = Text::new("   ", TextStyle::new(24.0));
        let tile = text.render(&mut reg);
        assert!(tile.is_empty());
        assert_eq!(tile.byte_len(), 0);
    }

    #[test]
    fn testo_su_una_riga_produce_pixel_del_colore_giusto() {
        let mut reg = registro_con_font().expect("font di sistema");
        let text = Text::new("Pizzart", TextStyle::new(48.0).color([200, 40, 20, 255]));
        let tile = text.render(&mut reg);
        assert!(!tile.is_empty(), "il testo deve produrre pixel");

        let mut trovati = 0;
        for px in tile.rgba.as_chunks::<4>().0 {
            if px[3] > 0 {
                trovati += 1;
                assert_eq!(px[0], 200, "canale R");
                assert_eq!(px[1], 40, "canale G");
                assert_eq!(px[2], 20, "canale B");
            }
        }
        assert!(trovati > 100, "pixel coperti: {trovati}");
    }

    #[test]
    fn testo_multilinea_aumenta_l_altezza() {
        let mut reg = registro_con_font().expect("font di sistema");
        let style = TextStyle::new(32.0);
        let una = Text::new("Riga", style.clone()).render(&mut reg);
        let tre = Text::new("Riga\nRiga\nRiga", style).render(&mut reg);
        assert!(
            tre.height > una.height * 2,
            "tre righe ({}) devono essere più alte di una ({})",
            tre.height,
            una.height
        );
    }

    #[test]
    fn wrapping_rispetta_il_bounds() {
        let mut reg = registro_con_font().expect("font di sistema");
        let testo = "una frase abbastanza lunga da dover andare a capo piu' volte";
        let style = TextStyle::new(24.0);
        let senza = Text::new(testo, style.clone()).render(&mut reg);
        let con = Text::new(testo, style)
            .with_bounds(200.0, 0.0)
            .render(&mut reg);

        assert!(
            con.width <= 220,
            "con bounds 200 la larghezza deve restare vicina: {}",
            con.width
        );
        assert!(
            con.height > senza.height,
            "andando a capo l'altezza deve crescere: {} vs {}",
            con.height,
            senza.height
        );
    }

    #[test]
    fn glifo_mancante_non_va_in_panico() {
        let mut reg = registro_con_font().expect("font di sistema");
        // caratteri non latini, probabilmente non nel font di prova
        let text = Text::new("あア漢字🎨", TextStyle::new(32.0));
        let tile = text.render(&mut reg);
        // l'importante è non andare in panico: il fallback decide lui
        assert!(tile.canvas_w < 10_000);
    }

    #[test]
    fn la_posizione_del_tile_segue_il_punto_scelto() {
        let mut reg = registro_con_font().expect("font di sistema");
        let a = Text::new("X", TextStyle::new(40.0))
            .at(10.0, 20.0)
            .render(&mut reg);
        let b = Text::new("X", TextStyle::new(40.0))
            .at(110.0, 220.0)
            .render(&mut reg);
        assert!(!a.is_empty() && !b.is_empty());
        assert_eq!(b.offset_x - a.offset_x, 100);
        assert_eq!(b.offset_y - a.offset_y, 200);
    }

    #[test]
    fn font_da_bytes_o_da_file_danno_lo_stesso_risultato() {
        let Some(path) = [
            r"C:\Windows\Fonts\arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
        .iter()
        .find(|p| std::path::Path::new(p).exists()) else {
            eprintln!("nessun font su disco: test saltato");
            return;
        };

        let mut da_file = FontRegistry::new();
        let id_file = da_file.add_file(*path).expect("da file");
        let family = da_file.get(id_file).unwrap().family.clone();

        let bytes = std::fs::read(path).expect("lettura");
        let mut da_bytes = FontRegistry::new();
        da_bytes.add(bytes, FontSource::Memory).expect("da bytes");

        let style = TextStyle::new(28.0).family(family);
        let a = Text::new("Confronto", style.clone()).render(&mut da_file);
        let b = Text::new("Confronto", style).render(&mut da_bytes);
        assert_eq!(a.rgba, b.rgba, "stessi byte di font, stesso risultato");
    }

    #[test]
    fn l_allineamento_sposta_il_testo() {
        let mut reg = registro_con_font().expect("font di sistema");
        let style = TextStyle::new(28.0);
        let sinistra = Text::new("Pizzart", style.clone())
            .with_bounds(300.0, 0.0)
            .render(&mut reg);
        let destra = Text::new("Pizzart", style.align(Align::Right))
            .with_bounds(300.0, 0.0)
            .render(&mut reg);
        assert!(
            destra.offset_x > sinistra.offset_x,
            "allineato a destra deve stare più a destra: {} vs {}",
            destra.offset_x,
            sinistra.offset_x
        );
    }
}
