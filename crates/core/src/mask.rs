//! Maschere: un canale alpha che decide **dove** un layer si vede.
//!
//! Una maschera non è un contenuto: è un modificatore. Non ha colore, non ha
//! blend, non produce pixel visibili. Ha una sola informazione per pixel —
//! quanto si vede — e questa semplicità è ciò che la rende componibile:
//! scegliere, disegnare, scrivere o caricare un'immagine produce **la stessa
//! maschera**, e la pipeline di compositing non cambia.
//!
//! ## Campionamento
//!
//! La maschera viene campionata con le UV del layer, quindi **una maschera di
//! dimensioni diverse viene adattata** alla dimensione del layer (nearest
//! sampling, bordi clampati). È un comportamento esplicito, non un errore
//! silenzioso: un'app che vuole 1:1 passa una maschera della stessa dimensione.

use std::sync::Arc;

use crate::image::PlacedImage;
use crate::layer::LayerId;

/// Da dove viene una maschera. Non cambia la composizione: serve alla UI e al debug.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MaskSource {
    /// Da uno strumento di selezione (rettangolo, lazo, bacchetta magica).
    Selection,
    /// Dal canale alpha di un altro layer.
    FromLayer(LayerId),
    /// Da una tessera qualsiasi: testo, forma, tratto, immagine.
    FromTile,
    /// Generata: gradiente, rumore, pennello (futuro).
    #[default]
    Procedural,
}

/// Un canale alpha, 8 bit per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    /// `width * height` byte. 0 = nascosto, 255 = visibile.
    pub alpha: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Se vero, il valore usato in composizione è `255 - alpha`.
    pub inverted: bool,
    pub source: MaskSource,
}

impl Mask {
    /// Costruisce una maschera da un canale già pronto.
    ///
    /// # Panics
    /// Se `alpha.len() != width * height`.
    pub fn new(alpha: Vec<u8>, width: u32, height: u32, source: MaskSource) -> Self {
        assert_eq!(
            alpha.len(),
            width as usize * height as usize,
            "la maschera deve avere un byte per pixel"
        );
        Self {
            alpha,
            width,
            height,
            inverted: false,
            source,
        }
    }

    /// Maschera che nasconde tutto.
    pub fn empty(width: u32, height: u32) -> Self {
        Self::new(
            vec![0u8; width as usize * height as usize],
            width,
            height,
            MaskSource::Procedural,
        )
    }

    /// Maschera che lascia passare tutto (l'elemento neutro).
    pub fn full(width: u32, height: u32) -> Self {
        Self::new(
            vec![255u8; width as usize * height as usize],
            width,
            height,
            MaskSource::Procedural,
        )
    }

    /// Da una tessera qualsiasi: si usa **solo** il canale alpha.
    ///
    /// La maschera mantiene le dimensioni della tessera; per applicarla a un
    /// layer più grande serve [`Mask::from_tile_placed`] oppure il ridimensionamento
    /// automatico del campionamento.
    pub fn from_tile(tile: &PlacedImage) -> Self {
        let mut alpha = Vec::with_capacity(tile.pixel_count());
        for px in tile.rgba.as_chunks::<4>().0 {
            alpha.push(px[3]);
        }
        Self {
            alpha,
            width: tile.width,
            height: tile.height,
            inverted: false,
            source: MaskSource::FromTile,
        }
    }

    /// Da una tessera, **posizionata** nella tela: la maschera ha le dimensioni
    /// della tela e il contenuto della tessera finisce al suo offset.
    ///
    /// È la forma che serve quasi sempre: il testo «PIZZART» è una tessera
    /// 424×75 a (71, 81), la maschera deve essere 900×520.
    pub fn from_tile_placed(tile: &PlacedImage, canvas_w: u32, canvas_h: u32) -> Self {
        let mut alpha = vec![0u8; canvas_w as usize * canvas_h as usize];
        for row in 0..tile.height {
            let y = tile.offset_y + row as i32;
            if y < 0 || y >= canvas_h as i32 {
                continue;
            }
            for col in 0..tile.width {
                let x = tile.offset_x + col as i32;
                if x < 0 || x >= canvas_w as i32 {
                    continue;
                }
                let src = ((row as usize * tile.width as usize) + col as usize) * 4;
                alpha[y as usize * canvas_w as usize + x as usize] = tile.rgba[src + 3];
            }
        }
        Self {
            alpha,
            width: canvas_w,
            height: canvas_h,
            inverted: false,
            source: MaskSource::FromTile,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.alpha.is_empty()
    }

    /// Valore della maschera alle UV normalizzate (0..1), con inversione applicata.
    ///
    /// Fuori dai bordi si usa il bordo (clamp to edge), come il sampler della GPU.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        if self.is_empty() {
            return if self.inverted { 1.0 } else { 0.0 };
        }
        let x = ((u * self.width as f32) as i32).clamp(0, self.width as i32 - 1) as usize;
        let y = ((v * self.height as f32) as i32).clamp(0, self.height as i32 - 1) as usize;
        let raw = self.alpha[y * self.width as usize + x] as f32 / 255.0;
        if self.inverted {
            1.0 - raw
        } else {
            raw
        }
    }

    /// Valore del pixel, con inversione applicata.
    pub fn value_at(&self, x: u32, y: u32) -> Option<f32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let raw = self.alpha[y as usize * self.width as usize + x as usize] as f32 / 255.0;
        Some(if self.inverted { 1.0 - raw } else { raw })
    }

    /// Cambia l'inversione; restituisce il valore precedente (per l'undo).
    pub fn set_inverted(&mut self, inverted: bool) -> bool {
        std::mem::replace(&mut self.inverted, inverted)
    }

    /// Copia con un'altra inversione, senza mutare l'originale.
    pub fn with_inverted(mut self, inverted: bool) -> Self {
        self.inverted = inverted;
        self
    }

    /// Byte totali (per stimare la memoria occupata).
    pub fn byte_len(&self) -> usize {
        self.alpha.len()
    }
}

/// Comodità: la maschera è condivisa fra documenti con `Arc`.
pub type SharedMask = Arc<Mask>;

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(alpha: &[u8], w: u32, h: u32) -> PlacedImage {
        let mut rgba = Vec::with_capacity(alpha.len() * 4);
        for a in alpha {
            rgba.extend_from_slice(&[10, 20, 30, *a]);
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

    #[test]
    fn full_ed_empty_sono_gli_estremi() {
        let pieno = Mask::full(4, 3);
        assert_eq!(pieno.sample(0.0, 0.0), 1.0);
        assert_eq!(pieno.sample(0.99, 0.99), 1.0);
        let vuoto = Mask::empty(4, 3);
        assert_eq!(vuoto.sample(0.0, 0.0), 0.0);
    }

    #[test]
    fn from_tile_estrae_solo_l_alpha() {
        let t = tile(&[0, 64, 128, 255], 4, 1);
        let m = Mask::from_tile(&t);
        assert_eq!(m.width, 4);
        assert_eq!(m.height, 1);
        assert_eq!(m.alpha, vec![0, 64, 128, 255]);
        assert_eq!(m.source, MaskSource::FromTile);
    }

    #[test]
    fn from_tile_placed_rispetta_l_offset() {
        let mut t = tile(&[255; 2], 2, 1);
        t.offset_x = 3;
        t.offset_y = 1;
        let m = Mask::from_tile_placed(&t, 6, 3);
        assert_eq!(m.width, 6);
        assert_eq!((m.width * m.height) as usize, m.alpha.len());
        assert_eq!(m.value_at(3, 1), Some(1.0));
        assert_eq!(m.value_at(4, 1), Some(1.0));
        assert_eq!(m.value_at(2, 1), Some(0.0), "prima dell'offset");
        assert_eq!(m.value_at(3, 0), Some(0.0), "sopra l'offset");
    }

    #[test]
    fn l_inversione_si_applica_al_campione() {
        let m = Mask::new(vec![200], 1, 1, MaskSource::Procedural);
        assert!((m.sample(0.5, 0.5) - 200.0 / 255.0).abs() < 1e-6);
        let inv = m.clone().with_inverted(true);
        assert!((inv.sample(0.5, 0.5) - 55.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn il_campionamento_adatta_dimensioni_diverse() {
        // maschera 2x1 su UV: il primo pixel copre u < 0.5
        let m = Mask::new(vec![0, 255], 2, 1, MaskSource::Procedural);
        assert_eq!(m.sample(0.1, 0.5), 0.0);
        assert_eq!(m.sample(0.6, 0.5), 1.0);
        assert_eq!(m.sample(-1.0, 0.5), 0.0, "clamp a sinistra");
        assert_eq!(m.sample(2.0, 0.5), 1.0, "clamp a destra");
    }

    #[test]
    fn set_inverted_restituisce_il_vecchio_valore() {
        let mut m = Mask::full(2, 2);
        assert!(!m.inverted);
        assert!(!m.set_inverted(true));
        assert!(m.inverted);
        assert!(m.set_inverted(false));
    }

    #[test]
    fn una_maschera_vuota_e_vuota() {
        assert!(Mask::empty(0, 0).is_empty());
        assert!(!Mask::full(1, 1).is_empty());
        assert_eq!(Mask::full(3, 2).byte_len(), 6);
    }
}
