//! Layer: identificativo stabile, pixel RGBA8, proprietà di fusione.
//!
//! `LayerId` è un contatore atomico invece di un UUID: nessuna dipendenza
//! esterna, valori leggibili nei log e nei test, e il documento resta
//! serializzabile come semplice numero.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::mask::Mask;
use crate::stable::StableId;

use crate::blend::BlendMode;
use crate::transform::Transform;

/// Identificativo di un layer, unico nel processo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(u64);

impl LayerId {
    /// Nuovo identificativo (monotono, mai riusato).
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn raw(self) -> u64 {
        self.0
    }

    /// Ricostruisce un identificativo da un valore serializzato.
    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

impl Default for LayerId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}", self.0)
    }
}

/// Un livello dell'immagine: pixel RGBA8 con alpha **non premoltiplicato**.
#[derive(Clone, Debug)]
pub struct Layer {
    /// Identificativo stabile nel file (diverso da `id`, che è un contatore).
    pub stable: StableId,
    pub id: LayerId,
    pub name: String,
    pub transform: Transform,
    /// 0.0 = invisibile, 1.0 = opaco.
    pub opacity: f32,
    pub blend: BlendMode,
    pub visible: bool,
    /// Maschera che decide dove il layer si vede (alpha moltiplicata).
    pub mask: Option<Arc<Mask>>,
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` byte, ordine RGBA.
    pub pixels: Vec<u8>,
}

impl Layer {
    /// Costruisce un layer verificando la dimensione del buffer.
    ///
    /// # Panics
    /// Se `pixels.len() != width * height * 4`.
    pub fn new(name: impl Into<String>, width: u32, height: u32, pixels: Vec<u8>) -> Self {
        assert_eq!(
            pixels.len(),
            (width as usize * height as usize * 4),
            "il buffer deve essere RGBA8: {width}x{height} -> {} byte",
            width as usize * height as usize * 4
        );
        Self {
            id: LayerId::new(),
            name: name.into(),
            transform: Transform::default(),
            opacity: 1.0,
            blend: BlendMode::Normal,
            visible: true,
            stable: StableId::new(),
            mask: None,
            width,
            height,
            pixels,
        }
    }

    /// Layer di un colore pieno (utile nei test e per "riempi livello").
    pub fn filled(name: impl Into<String>, width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..(width as usize * height as usize) {
            pixels.extend_from_slice(&rgba);
        }
        Self::new(name, width, height, pixels)
    }

    /// Layer completamente trasparente.
    pub fn transparent(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self::new(
            name,
            width,
            height,
            vec![0u8; width as usize * height as usize * 4],
        )
    }

    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((y as usize * self.width as usize + x as usize) * 4)
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let i = self.index(x, y)?;
        Some([
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ])
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) -> bool {
        match self.index(x, y) {
            Some(i) => {
                self.pixels[i..i + 4].copy_from_slice(&rgba);
                true
            }
            None => false,
        }
    }

    /// Imposta l'opacità e restituisce il valore precedente (per l'undo).
    pub fn set_opacity(&mut self, value: f32) -> f32 {
        let old = self.opacity;
        self.opacity = value.clamp(0.0, 1.0);
        old
    }

    /// Il layer occupa area visibile? (scala non nulla e alpha non zero)
    pub fn is_drawable(&self) -> bool {
        self.visible
            && self.opacity > 0.0
            && self.transform.scale_x.abs() > f32::EPSILON
            && self.transform.scale_y.abs() > f32::EPSILON
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gli_identificativi_sono_unici_e_crescenti() {
        let a = LayerId::new();
        let b = LayerId::new();
        assert_ne!(a, b);
        assert!(b.raw() > a.raw());
    }

    #[test]
    fn from_raw_ripristina_il_valore() {
        let id = LayerId::from_raw(42);
        assert_eq!(id.raw(), 42);
        assert_eq!(id.to_string(), "L42");
    }

    #[test]
    fn il_buffer_deve_avere_la_dimensione_giusta() {
        let r = std::panic::catch_unwind(|| Layer::new("x", 10, 10, vec![0; 10]));
        assert!(r.is_err(), "un buffer troppo corto deve essere rifiutato");
    }

    #[test]
    fn pixel_in_lettura_e_scrittura() {
        let mut l = Layer::transparent("x", 4, 3);
        assert!(l.set_pixel(3, 2, [10, 20, 30, 255]));
        assert_eq!(l.pixel(3, 2), Some([10, 20, 30, 255]));
        assert_eq!(l.pixel(4, 2), None, "fuori dal bordo");
        assert!(!l.set_pixel(0, 3, [1, 2, 3, 4]), "_fuori dal bordo");
    }

    #[test]
    fn il_layer_pieno_ha_tutti_i_pixel_uguali() {
        let l = Layer::filled("rosso", 3, 2, [255, 0, 0, 255]);
        assert_eq!(l.byte_len(), 3 * 2 * 4);
        assert_eq!(l.pixel(2, 1), Some([255, 0, 0, 255]));
    }

    #[test]
    fn l_opacita_e_limitata_e_restituisce_il_vecchio_valore() {
        let mut l = Layer::transparent("x", 1, 1);
        assert_eq!(l.opacity, 1.0);
        assert_eq!(l.set_opacity(0.25), 1.0);
        assert_eq!(l.opacity, 0.25);
        l.set_opacity(5.0);
        assert_eq!(l.opacity, 1.0, "clamp a 1");
        l.set_opacity(-2.0);
        assert_eq!(l.opacity, 0.0, "clamp a 0");
    }

    #[test]
    fn un_layer_a_scala_zero_non_e_disegnabile() {
        let mut l = Layer::filled("x", 1, 1, [0, 0, 0, 255]);
        assert!(l.is_drawable());
        l.transform.scale_x = 0.0;
        assert!(!l.is_drawable());
    }
}
