// Shader: effetti point-wise (un pixel in ingresso, un pixel in uscita).
//
// Tre entry point isolati — `fs_brightness`, `fs_contrast`, `fs_saturate` —
// condividono solo le funzioni di supporto. Ogni effetto è un pass a sé: il
// renderer li mette in catena con ping-pong.
//
// Contratto comune a tutti gli effetti:
//   1. si legge il pixel corrente;
//   2. si trasforma il colore (RGB) secondo l'effetto;
//   3. si compone il risultato sopra il pixel corrente con `blend` e `opacity`;
//   4. l'alpha resta quello dell'ingresso: un effetto cambia il colore, non la
//      copertura della tessera.

struct Fx {
    params: vec4<f32>,
    blend: u32,
    opacity: f32,
    /// 1 = questo pass applica blend e opacità (è l'ultimo dell'effetto).
    is_last: f32,
    /// Raggio del kernel interno (0 = nessun kernel).
    radius: f32,
};

@group(0) @binding(0) var<uniform> u: Fx;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
/// Originale dell'effetto: serve solo ai pass di combinazione
/// (sharpen, glow), che confrontano il risultato con l'ingresso.
@group(0) @binding(3) var orig_tex: texture_2d<f32>;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );
    var out: VOut;
    out.pos = vec4<f32>(corners[vi], 0.0, 1.0);
    out.uv = uvs[vi];
    return out;
}

fn clamp01(v: f32) -> f32 {
    return clamp(v, 0.0, 1.0);
}

// Stesse formule del compositing (e del core): un modo, una definizione.
fn blend_channel(mode: u32, cb_in: f32, cs_in: f32) -> f32 {
    let cb = clamp01(cb_in);
    let cs = clamp01(cs_in);
    var v = cs;

    if (mode == 1u) {
        v = cb * cs;
    } else if (mode == 2u) {
        v = cb + cs - cb * cs;
    } else if (mode == 3u) {
        if (cb <= 0.5) { v = 2.0 * cb * cs; }
        else { v = 1.0 - 2.0 * (1.0 - cb) * (1.0 - cs); }
    } else if (mode == 4u) {
        v = min(cb, cs);
    } else if (mode == 5u) {
        v = max(cb, cs);
    } else if (mode == 6u) {
        if (cb <= 0.0) { v = 0.0; }
        else if (cs >= 1.0) { v = 1.0; }
        else { v = min(cb / (1.0 - cs), 1.0); }
    } else if (mode == 7u) {
        if (cb >= 1.0) { v = 1.0; }
        else if (cs <= 0.0) { v = 0.0; }
        else { v = 1.0 - min((1.0 - cb) / cs, 1.0); }
    } else if (mode == 8u) {
        if (cs <= 0.5) { v = 2.0 * cs * cb; }
        else { v = 1.0 - 2.0 * (1.0 - cs) * (1.0 - cb); }
    } else if (mode == 9u) {
        var d = sqrt(cb);
        if (cb <= 0.25) { d = ((16.0 * cb - 12.0) * cb + 4.0) * cb; }
        if (cs <= 0.5) { v = cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb); }
        else { v = cb + (2.0 * cs - 1.0) * (d - cb); }
    } else if (mode == 10u) {
        v = abs(cb - cs);
    } else if (mode == 11u) {
        v = cb + cs - 2.0 * cb * cs;
    }
    return clamp01(v);
}

fn effect_color(kind: u32, c: vec3<f32>) -> vec3<f32> {
    if (kind == 0u) {
        // brightness: somma il delta, clamp
        return clamp(c + vec3<f32>(u.params.x, u.params.x, u.params.x), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    if (kind == 1u) {
        // contrast: allontana dal grigio medio
        let f = 1.0 + u.params.x;
        return clamp((c - vec3<f32>(0.5)) * vec3<f32>(f, f, f) + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    // saturate: interpola fra la luminanza e il colore
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let f = 1.0 + u.params.x;
    return clamp(vec3<f32>(l, l, l) + (c - vec3<f32>(l, l, l)) * vec3<f32>(f, f, f), vec3<f32>(0.0), vec3<f32>(1.0));
}

/// Compone un colore sopra il pixel di partenza con blend e opacità dell'effetto.
fn compose_over(prev: vec4<f32>, wanted: vec3<f32>) -> vec4<f32> {
    var out = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0; i < 3; i = i + 1) {
        let cb = clamp01(prev[i]);
        let b = blend_channel(u.blend, cb, wanted[i]);
        out[i] = clamp01(cb + (b - cb) * clamp01(u.opacity));
    }
    return vec4<f32>(out, prev.a);
}

fn compose(kind: u32, uv: vec2<f32>) -> vec4<f32> {
    let prev = textureSample(tex, samp, uv);
    let fx = effect_color(kind, prev.rgb);
    return compose_over(prev, fx);
}

// ---------------------------------------------------------------------------
// Blur separabile: due pass, orizzontale poi verticale.
//
// Un unico pass con raggio r costa (2r+1)^2 campioni per pixel; separato ne
// costa 2*(2r+1). Il renderer esegue `fs_blur_h` e poi `fs_blur_v`, quindi la
// texture intermedia è già sfocata in orizzontale quando parte la verticale.
//
// `params[0]` = raggio in pixel (0..16), `params[1]` = sigma (<=0 -> automatico),
// `params[2]` = "componi" (1 = ultimo pass, 0 = pass intermedio).
//
// Senza questo flag il blur applicherebbe blend e opacità **due volte** (una per
// pass): con Normal/1.0 non si vedrebbe, con Multiply/0.5 sarebbe sbagliato.
// Campionamento nearest, come la CPU: la parità è un contratto, non un caso.
// ---------------------------------------------------------------------------

const MAX_BLUR_RADIUS: f32 = 16.0;

fn gaussian(sigma: f32, i: f32) -> f32 {
    return exp(-(i * i) / (2.0 * sigma * sigma));
}

fn blur_pass(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
    let prev = textureSampleLevel(tex, samp, uv, 0.0);
    let radius = clamp(u.radius, 0.0, MAX_BLUR_RADIUS);
    let do_compose = u.is_last > 0.5;

    if (radius < 0.5) {
        // niente da sfocare: l'effetto è l'identità (ma il blend vale comunque)
        if (!do_compose) {
            return prev;
        }
        return compose_over(prev, prev.rgb);
    }

    var sigma = u.params[1];
    if (sigma <= 0.0) {
        sigma = max(radius * 0.5, 0.5);
    }

    let dims = vec2<f32>(textureDimensions(tex));
    let texel = vec2<f32>(1.0, 1.0) / dims;
    let r = i32(ceil(radius));

    var accum = vec3<f32>(0.0, 0.0, 0.0);
    var weight_sum = 0.0;
    for (var i = -r; i <= r; i = i + 1) {
        let fi = f32(i);
        let w = gaussian(sigma, fi);
        let sample_uv = uv + direction * texel * fi;
        accum = accum + textureSampleLevel(tex, samp, sample_uv, 0.0).rgb * w;
        weight_sum = weight_sum + w;
    }
    let blurred = accum / weight_sum;

    if (!do_compose) {
        // pass intermedio: si passa solo il colore sfocato, alpha invariato
        return vec4<f32>(blurred, prev.a);
    }

    return compose_over(prev, blurred);
}

// ---------------------------------------------------------------------------
// Combinazioni a due ingressi (kernel + originale).
//
// Sono il terzo pass di sharpen e glow: `tex` contiene il risultato del kernel,
// `orig_tex` l'ingresso dell'effetto. Servono due texture perché il risultato
// non è una trasformazione del solo sfocato: è un confronto con l'originale.
// ---------------------------------------------------------------------------

/// Nitidezza (unsharp mask): `out = orig + amount * (orig - blurred)`.
@fragment
fn fs_sharpen_combine(in: VOut) -> @location(0) vec4<f32> {
    let orig = textureSampleLevel(orig_tex, samp, in.uv, 0.0);
    let blurred = textureSampleLevel(tex, samp, in.uv, 0.0);
    let amount = u.params.x;
    let sharp = clamp(orig.rgb + (orig.rgb - blurred.rgb) * amount, vec3<f32>(0.0), vec3<f32>(1.0));
    return compose_over(orig, sharp);
}

/// Bagliore: aggiunge con Screen la parte di sfocato sopra la soglia.
@fragment
fn fs_glow_combine(in: VOut) -> @location(0) vec4<f32> {
    let orig = textureSampleLevel(orig_tex, samp, in.uv, 0.0);
    let blurred = textureSampleLevel(tex, samp, in.uv, 0.0);
    let threshold = u.params.x;
    let intensity = u.params.y;
    let excess = max(vec3<f32>(0.0), blurred.rgb - vec3<f32>(threshold)) * intensity;

    var glowing = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0; i < 3; i = i + 1) {
        glowing[i] = blend_channel(2u, orig[i], excess[i]); // 2u = Screen
    }
    return compose_over(orig, clamp(glowing, vec3<f32>(0.0), vec3<f32>(1.0)));
}

@fragment
fn fs_blur_h(in: VOut) -> @location(0) vec4<f32> {
    return blur_pass(in.uv, vec2<f32>(1.0, 0.0));
}

@fragment
fn fs_blur_v(in: VOut) -> @location(0) vec4<f32> {
    return blur_pass(in.uv, vec2<f32>(0.0, 1.0));
}

@fragment
fn fs_brightness(in: VOut) -> @location(0) vec4<f32> {
    return compose(0u, in.uv);
}

@fragment
fn fs_contrast(in: VOut) -> @location(0) vec4<f32> {
    return compose(1u, in.uv);
}

@fragment
fn fs_saturate(in: VOut) -> @location(0) vec4<f32> {
    return compose(2u, in.uv);
}
