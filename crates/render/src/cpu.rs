//! Riferimento CPU degli effetti.
//!
//! Stessa matematica dello shader `effects.wgsl`, ma in Rust: serve ai test di
//! parità, all'export senza GPU (CI, server) e come documentazione eseguibile
//! di cosa deve fare ogni effetto.

use pizzart_core::blend::blend_channel;
use pizzart_core::{BlendMode, PlacedImage};

use crate::{Effect, EffectShader};

fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Trasformazione del colore di un solo pixel.
///
/// `params[0]` è il parametro di ogni effetto semplice:
/// delta per la luminosità, fattore per contrasto e saturazione.
/// Il blur non è point-wise: qui restituisce il colore invariato, ed è
/// `blur_effect` a occuparsene.
pub fn effect_rgb(shader: EffectShader, rgb: [f32; 3], params: [f32; 4]) -> [f32; 3] {
    let p = params[0];
    let out = match shader {
        EffectShader::Brightness => [rgb[0] + p, rgb[1] + p, rgb[2] + p],
        EffectShader::Contrast => {
            let f = 1.0 + p;
            [
                (rgb[0] - 0.5) * f + 0.5,
                (rgb[1] - 0.5) * f + 0.5,
                (rgb[2] - 0.5) * f + 0.5,
            ]
        }
        EffectShader::Saturate => {
            let l = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
            let f = 1.0 + p;
            [
                l + (rgb[0] - l) * f,
                l + (rgb[1] - l) * f,
                l + (rgb[2] - l) * f,
            ]
        }
        EffectShader::Blur => rgb,
        // Sharpen e glow non sono point-wise: li gestiscono le funzioni sotto.
        EffectShader::Sharpen | EffectShader::Glow => rgb,
    };
    [clamp01(out[0]), clamp01(out[1]), clamp01(out[2])]
}

/// Nitidezza (unsharp mask) su CPU: blur separabile, poi `orig + amount * (orig - blurred)`.
fn sharpen_effect(rgba: &[u8], w: u32, h: u32, effect: &Effect) -> Vec<u8> {
    let horizontal = blur_direction(rgba, w, h, effect.radius, 0.0, 1, 0);
    let vertical = blur_direction(&horizontal, w, h, effect.radius, 0.0, 0, 1);
    let amount = effect.params[0];
    let opacity = clamp01(effect.opacity);

    let mut out = rgba.to_vec();
    for i in (0..out.len()).step_by(4) {
        for c in 0..3 {
            let orig = clamp01(rgba[i + c] as f32 / 255.0);
            let blurred = clamp01(vertical[i + c] as f32 / 255.0);
            let sharp = clamp01(orig + (orig - blurred) * amount);
            let blended = blend_channel(effect.blend, orig, sharp);
            let value = clamp01(orig + (blended - orig) * opacity);
            out[i + c] = (value * 255.0 + 0.5) as u8;
        }
        out[i + 3] = rgba[i + 3];
    }
    out
}

/// Bagliore su CPU: sfocato sopra soglia, aggiunto con Screen all'originale.
fn glow_effect(rgba: &[u8], w: u32, h: u32, effect: &Effect) -> Vec<u8> {
    let horizontal = blur_direction(rgba, w, h, effect.radius, 0.0, 1, 0);
    let vertical = blur_direction(&horizontal, w, h, effect.radius, 0.0, 0, 1);
    let threshold = effect.params[0];
    let intensity = effect.params[1];
    let opacity = clamp01(effect.opacity);

    let mut out = rgba.to_vec();
    for i in (0..out.len()).step_by(4) {
        for c in 0..3 {
            let orig = clamp01(rgba[i + c] as f32 / 255.0);
            let blurred = clamp01(vertical[i + c] as f32 / 255.0);
            let excess = ((blurred - threshold).max(0.0)) * intensity;
            let glowing = clamp01(blend_channel(BlendMode::Screen, orig, excess));
            let blended = blend_channel(effect.blend, orig, glowing);
            let value = clamp01(orig + (blended - orig) * opacity);
            out[i + c] = (value * 255.0 + 0.5) as u8;
        }
        out[i + 3] = rgba[i + 3];
    }
    out
}

/// Convoluzione gaussiana in una direzione, con bordi "clamp to edge".
///
/// Quantizza a 8 bit in uscita, come la texture intermedia della GPU: senza
/// questa quantizzazione la parità sarebbe bugiarda.
fn blur_direction(
    rgba: &[u8],
    w: u32,
    h: u32,
    radius: f32,
    sigma: f32,
    dx: i32,
    dy: i32,
) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let r = radius.clamp(0.0, 16.0).ceil() as i32;
    if r == 0 {
        return out;
    }
    let sigma = if sigma > 0.0 {
        sigma
    } else {
        (radius * 0.5).max(0.5)
    };

    let mut weights = Vec::with_capacity((2 * r + 1) as usize);
    let mut weight_sum = 0.0f32;
    for i in -r..=r {
        let fi = i as f32;
        let weight = (-(fi * fi) / (2.0 * sigma * sigma)).exp();
        weights.push(weight);
        weight_sum += weight;
    }

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut acc = [0.0f32; 3];
            for (index, i) in (-r..=r).enumerate() {
                let sx = (x + i * dx).clamp(0, w as i32 - 1) as u32;
                let sy = (y + i * dy).clamp(0, h as i32 - 1) as u32;
                let si = ((sy as usize * w as usize) + sx as usize) * 4;
                let weight = weights[index];
                for c in 0..3 {
                    acc[c] += (rgba[si + c] as f32 / 255.0) * weight;
                }
            }
            let di = ((y as usize * w as usize) + x as usize) * 4;
            for c in 0..3 {
                let value = clamp01(acc[c] / weight_sum);
                out[di + c] = (value * 255.0 + 0.5) as u8;
            }
            // l'alpha non viene sfocato: un effetto cambia il colore, non la copertura
        }
    }
    out
}

/// Blur completo: orizzontale, verticale, poi composizione su un solo pass.
///
/// La composizione avviene fra il risultato e l'**uscita del primo pass**
/// (non l'originale): è esattamente quello che fa la GPU, dove l'ultimo pass
/// legge la texture già sfocata in orizzontale.
fn blur_effect(rgba: &[u8], w: u32, h: u32, effect: &Effect) -> Vec<u8> {
    let radius = effect.params[0];
    let sigma = effect.params[1];
    let horizontal = blur_direction(rgba, w, h, radius, sigma, 1, 0);
    let vertical = blur_direction(&horizontal, w, h, radius, sigma, 0, 1);

    let mut out = vertical.clone();
    let opacity = clamp01(effect.opacity);
    for i in (0..out.len()).step_by(4) {
        for c in 0..3 {
            let cb = clamp01(horizontal[i + c] as f32 / 255.0);
            let cs = clamp01(vertical[i + c] as f32 / 255.0);
            let blended = blend_channel(effect.blend, cb, cs);
            let value = clamp01(cb + (blended - cb) * opacity);
            out[i + c] = (value * 255.0 + 0.5) as u8;
        }
        out[i + 3] = horizontal[i + 3];
    }
    out
}

/// Applica un effetto a una tessera, restituendone una nuova.
///
/// L'ingresso non viene toccato; posizione e tela di riferimento sono conservate.
pub fn apply_effect(input: &PlacedImage, effect: &Effect) -> PlacedImage {
    apply_effects(input, std::slice::from_ref(effect))
}

/// Applica una catena di effetti, nell'ordine dato.
pub fn apply_effects(input: &PlacedImage, effects: &[Effect]) -> PlacedImage {
    if input.is_empty() || effects.is_empty() {
        return input.clone();
    }

    let mut rgba = input.rgba.clone();
    for effect in effects {
        match effect.shader {
            EffectShader::Blur => {
                rgba = blur_effect(&rgba, input.width, input.height, effect);
                continue;
            }
            EffectShader::Sharpen => {
                rgba = sharpen_effect(&rgba, input.width, input.height, effect);
                continue;
            }
            EffectShader::Glow => {
                rgba = glow_effect(&rgba, input.width, input.height, effect);
                continue;
            }
            _ => {}
        }
        let opacity = clamp01(effect.opacity);
        for px in rgba.chunks_exact_mut(4) {
            let prev = [
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
            ];
            let fx = effect_rgb(effect.shader, prev, effect.params);
            for c in 0..3 {
                let blended = blend_channel(effect.blend, prev[c], fx[c]);
                // opacità dell'effetto: 0 = ingresso intatto, 1 = effetto pieno
                let value = clamp01(prev[c] + (blended - prev[c]) * opacity);
                px[c] = (value * 255.0 + 0.5) as u8;
            }
            // l'alpha resta quello dell'ingresso: un effetto cambia il colore,
            // non la copertura.
        }
    }

    PlacedImage {
        rgba,
        width: input.width,
        height: input.height,
        offset_x: input.offset_x,
        offset_y: input.offset_y,
        canvas_w: input.canvas_w,
        canvas_h: input.canvas_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(rgba: Vec<u8>, w: u32, h: u32) -> PlacedImage {
        PlacedImage {
            rgba,
            width: w,
            height: h,
            offset_x: 3,
            offset_y: 7,
            canvas_w: 64,
            canvas_h: 64,
        }
    }

    #[test]
    fn la_luminosita_si_somma_al_colore() {
        let fx = Effect::brightness(0.2);
        let out = effect_rgb(fx.shader, [0.5, 0.5, 0.5], fx.params);
        assert!((out[0] - 0.7).abs() < 1e-6);
        let scuro = Effect::brightness(-0.2);
        let out = effect_rgb(scuro.shader, [0.5, 0.5, 0.5], scuro.params);
        assert!((out[0] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn il_contrasto_zero_non_cambia_nulla() {
        let fx = Effect::contrast(0.0);
        for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let out = effect_rgb(fx.shader, [v, v, v], fx.params);
            assert!(
                (out[0] - v).abs() < 1e-6,
                "contrasto 0 su {v} ha dato {}",
                out[0]
            );
        }
    }

    #[test]
    fn la_saturazione_a_meno_uno_da_la_luminanza() {
        let fx = Effect::saturate(-1.0);
        let out = effect_rgb(fx.shader, [1.0, 0.0, 0.0], fx.params);
        let l = 0.2126;
        assert!((out[0] - l).abs() < 1e-6);
        assert!((out[1] - l).abs() < 1e-6);
        assert!((out[2] - l).abs() < 1e-6);
    }

    #[test]
    fn la_catena_conserva_posizione_e_tela() {
        let input = tile(vec![128, 128, 128, 255], 1, 1);
        let out = apply_effects(&input, &[Effect::brightness(0.1), Effect::contrast(0.2)]);
        assert_eq!(out.offset_x, 3);
        assert_eq!(out.offset_y, 7);
        assert_eq!(out.canvas_w, 64);
        assert_eq!(out.width, 1);
    }

    #[test]
    fn una_catena_vuota_restituisce_una_copia() {
        let input = tile(vec![10, 20, 30, 255], 1, 1);
        let out = apply_effects(&input, &[]);
        assert_eq!(out, input);
    }
}
