//! I 12 blend modes, come funzioni pure.
//!
//! Le formule sono quelle standard del W3C Compositing and Blending Level 1:
//! per ogni canale di colore si calcola `B(Cb, Cs)` (backdrop, source) e poi si
//! compone il tutto **in alpha non premoltiplicato**.
//!
//! Tenere la matematica qui, separata dal renderer, significa:
//! - si testa senza GPU;
//! - la stessa formula si replica identica in WGSL (stessi valori, stessi bordi);
//! - il riferimento CPU diventa il test di accettazione della versione GPU.

/// I modi di fusione esposti nell'interfaccia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
}

impl BlendMode {
    /// Tutti i modi, nell'ordine in cui vanno mostrati nel pannello.
    pub const ALL: [BlendMode; 12] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
    ];

    /// Etichetta italiana mostrata nel pannello livelli.
    pub fn label_it(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normale",
            BlendMode::Multiply => "Moltiplica",
            BlendMode::Screen => "Schiarisci",
            BlendMode::Overlay => "Sovrapponi",
            BlendMode::Darken => "Scurisci",
            BlendMode::Lighten => "Schiarisci al massimo",
            BlendMode::ColorDodge => "Scolora",
            BlendMode::ColorBurn => "Brucia",
            BlendMode::HardLight => "Luce forte",
            BlendMode::SoftLight => "Luce soffusa",
            BlendMode::Difference => "Differenza",
            BlendMode::Exclusion => "Esclusione",
        }
    }

    /// Identificativo stabile per serializzare il documento (non cambia mai).
    pub fn key(self) -> &'static str {
        match self {
            BlendMode::Normal => "normal",
            BlendMode::Multiply => "multiply",
            BlendMode::Screen => "screen",
            BlendMode::Overlay => "overlay",
            BlendMode::Darken => "darken",
            BlendMode::Lighten => "lighten",
            BlendMode::ColorDodge => "color-dodge",
            BlendMode::ColorBurn => "color-burn",
            BlendMode::HardLight => "hard-light",
            BlendMode::SoftLight => "soft-light",
            BlendMode::Difference => "difference",
            BlendMode::Exclusion => "exclusion",
        }
    }
}

fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// `B(Cb, Cs)` per un singolo canale, con Cb = backdrop e Cs = source.
pub fn blend_channel(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    let cb = clamp01(cb);
    let cs = clamp01(cs);
    let v = match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => cb * cs,
        BlendMode::Screen => cb + cs - cb * cs,
        // Overlay = HardLight con i ruoli scambiati.
        BlendMode::Overlay => {
            if cb <= 0.5 {
                2.0 * cb * cs
            } else {
                1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
            }
        }
        BlendMode::Darken => cb.min(cs),
        BlendMode::Lighten => cb.max(cs),
        BlendMode::ColorDodge => {
            if cb <= 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        BlendMode::ColorBurn => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        BlendMode::HardLight => {
            if cs <= 0.5 {
                2.0 * cs * cb
            } else {
                1.0 - 2.0 * (1.0 - cs) * (1.0 - cb)
            }
        }
        BlendMode::SoftLight => {
            // D(Cb) ufficiale: parte polinomiale sotto 0.25, radice sopra.
            let d = if cb <= 0.25 {
                ((16.0 * cb - 12.0) * cb + 4.0) * cb
            } else {
                cb.sqrt()
            };
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        BlendMode::Difference => (cb - cs).abs(),
        BlendMode::Exclusion => cb + cs - 2.0 * cb * cs,
    };
    clamp01(v)
}

/// Fusione di due pixel RGBA **non premoltiplicati** in `[0,1]`.
///
/// `opacity` è l'opacità del layer: riduce l'alpha della sorgente, come lo slider.
pub fn blend_rgba(mode: BlendMode, backdrop: [f32; 4], source: [f32; 4], opacity: f32) -> [f32; 4] {
    let opacity = clamp01(opacity);
    let ab = clamp01(backdrop[3]);
    let a_s = clamp01(source[3]) * opacity;

    let out_a = a_s + ab * (1.0 - a_s);
    if out_a <= f32::EPSILON {
        return [0.0, 0.0, 0.0, 0.0];
    }

    let mut out = [0.0f32; 3];
    for c in 0..3 {
        let cb = clamp01(backdrop[c]);
        let cs = clamp01(source[c]);
        let b = blend_channel(mode, cb, cs);
        // Composizione "source-over" con il termine di fusione del W3C.
        let co = a_s * (1.0 - ab) * cs + a_s * ab * b + (1.0 - a_s) * ab * cb;
        out[c] = clamp01(co / out_a);
    }
    [out[0], out[1], out[2], out_a]
}

/// Fusione in-place di due pixel RGBA8 (straight alpha), senza allocazioni.
pub fn blend_px(dst: &mut [u8], src: &[u8], mode: BlendMode, opacity: f32) {
    debug_assert!(dst.len() >= 4 && src.len() >= 4);
    let to_f = |b: u8| b as f32 / 255.0;
    let backdrop = [to_f(dst[0]), to_f(dst[1]), to_f(dst[2]), to_f(dst[3])];
    let source = [to_f(src[0]), to_f(src[1]), to_f(src[2]), to_f(src[3])];
    let out = blend_rgba(mode, backdrop, source, opacity);
    let to_u = |v: f32| (clamp01(v) * 255.0 + 0.5) as u8;
    dst[0] = to_u(out[0]);
    dst[1] = to_u(out[1]);
    dst[2] = to_u(out[2]);
    dst[3] = to_u(out[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, what: &str) {
        assert!((a - b).abs() < 1e-4, "{what}: {a} != {b}");
    }

    #[test]
    fn ci_sono_dodici_modi_con_chiavi_uniche() {
        assert_eq!(BlendMode::ALL.len(), 12);
        let mut keys: Vec<&str> = BlendMode::ALL.iter().map(|m| m.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 12, "chiavi duplicate");
    }

    #[test]
    fn multiply_bianco_non_cambia_il_backdrop() {
        for cb in [0.0, 0.3, 0.5, 0.9, 1.0] {
            approx(blend_channel(BlendMode::Multiply, cb, 1.0), cb, "per 1");
        }
        approx(blend_channel(BlendMode::Multiply, 0.5, 0.0), 0.0, "per 0");
    }

    #[test]
    fn screen_nero_non_cambia_il_backdrop() {
        for cb in [0.0, 0.25, 0.5, 1.0] {
            approx(blend_channel(BlendMode::Screen, cb, 0.0), cb, "piu' 0");
        }
        approx(blend_channel(BlendMode::Screen, 0.5, 1.0), 1.0, "piu' 1");
    }

    #[test]
    fn overlay_con_grigio_medio_e_identita() {
        for cb in [0.0, 0.1, 0.5, 0.75, 1.0] {
            approx(
                blend_channel(BlendMode::Overlay, cb, 0.5),
                cb,
                "identita' a 0.5",
            );
        }
    }

    #[test]
    fn dodge_e_burn_non_producono_nan_nei_bordi() {
        for mode in [BlendMode::ColorDodge, BlendMode::ColorBurn] {
            for cb in [0.0, 0.5, 1.0] {
                for cs in [0.0, 0.5, 1.0] {
                    let v = blend_channel(mode, cb, cs);
                    assert!(
                        v.is_finite() && (0.0..=1.0).contains(&v),
                        "{mode:?} {cb} {cs} -> {v}"
                    );
                }
            }
        }
        approx(
            blend_channel(BlendMode::ColorDodge, 0.0, 1.0),
            0.0,
            "dodge su nero",
        );
        approx(
            blend_channel(BlendMode::ColorBurn, 1.0, 0.0),
            1.0,
            "burn su bianco",
        );
    }

    #[test]
    fn softlight_a_grigio_medio_e_identita() {
        for cb in [0.0, 0.2, 0.5, 0.8, 1.0] {
            approx(
                blend_channel(BlendMode::SoftLight, cb, 0.5),
                cb,
                "identita' a 0.5",
            );
        }
    }

    #[test]
    fn differenza_ed_esclusione_simmetriche() {
        for (cb, cs) in [(0.2, 0.7), (0.5, 0.5), (0.9, 0.1)] {
            approx(
                blend_channel(BlendMode::Difference, cb, cs),
                blend_channel(BlendMode::Difference, cs, cb),
                "differenza",
            );
            approx(
                blend_channel(BlendMode::Exclusion, cb, cs),
                blend_channel(BlendMode::Exclusion, cs, cb),
                "esclusione",
            );
        }
    }

    #[test]
    fn tutti_i_modi_restano_nell_intervallo() {
        let steps = [0.0, 0.13, 0.25, 0.5, 0.77, 1.0];
        for mode in BlendMode::ALL {
            for cb in steps {
                for cs in steps {
                    let v = blend_channel(mode, cb, cs);
                    assert!((0.0..=1.0).contains(&v), "{mode:?} {cb} {cs} -> {v}");
                }
            }
        }
    }

    #[test]
    fn sorgente_trasparente_lascia_il_backdrop() {
        let backdrop = [0.2, 0.4, 0.6, 1.0];
        let source = [1.0, 0.0, 0.0, 0.0];
        for mode in BlendMode::ALL {
            let out = blend_rgba(mode, backdrop, source, 1.0);
            approx(out[0], 0.2, "r");
            approx(out[1], 0.4, "g");
            approx(out[2], 0.6, "b");
            approx(out[3], 1.0, "a");
        }
    }

    #[test]
    fn normale_opaca_copre_il_backdrop() {
        let out = blend_rgba(
            BlendMode::Normal,
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 0.5, 0.25, 1.0],
            1.0,
        );
        approx(out[0], 1.0, "r");
        approx(out[1], 0.5, "g");
        approx(out[2], 0.25, "b");
        approx(out[3], 1.0, "a");
    }

    #[test]
    fn opacita_di_layer_media_il_risultato() {
        // nero al 50% sopra bianco opaco -> grigio 0.5
        let out = blend_rgba(
            BlendMode::Normal,
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            0.5,
        );
        approx(out[0], 0.5, "grigio medio");
        approx(out[3], 1.0, "alpha resta opaco");
    }

    #[test]
    fn blend_px_lavora_su_rgba8() {
        let mut dst = [255u8, 255, 255, 255];
        let src = [0u8, 0, 0, 255];
        blend_px(&mut dst, &src, BlendMode::Normal, 0.5);
        assert_eq!(dst, [128, 128, 128, 255]);
    }
}
