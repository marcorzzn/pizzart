// Shader: WGSL del compositing.
//
// Ricalca una per una le formule di `pizzart-core/src/blend.rs`: ogni blend mode
// qui deve produrre lo stesso valore della versione CPU, altrimenti il test di
// parità (render::tests::gpu_e_cpu_concordano) fallisce.

struct Uniforms {
    // Da pixel del documento a pixel del layer (inversa della trasformazione).
    inv: mat4x4<f32>,
    doc_size: vec2<f32>,
    layer_size: vec2<f32>,
    mode: u32,
    opacity: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var prev_tex: texture_2d<f32>;
@group(0) @binding(2) var layer_tex: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
// Maschera del layer: canale alpha 8 bit. Se il layer non ha maschera si lega
// una texture 1x1 bianca, cosi` lo shader ha un solo percorso.
@group(0) @binding(4) var mask_tex: texture_2d<f32>;

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

fn blend_channel(mode: u32, cb_in: f32, cs_in: f32) -> f32 {
    let cb = clamp01(cb_in);
    let cs = clamp01(cs_in);
    var v = cs;

    if (mode == 1u) {                       // multiply
        v = cb * cs;
    } else if (mode == 2u) {                // screen
        v = cb + cs - cb * cs;
    } else if (mode == 3u) {                // overlay
        if (cb <= 0.5) { v = 2.0 * cb * cs; }
        else { v = 1.0 - 2.0 * (1.0 - cb) * (1.0 - cs); }
    } else if (mode == 4u) {                // darken
        v = min(cb, cs);
    } else if (mode == 5u) {                // lighten
        v = max(cb, cs);
    } else if (mode == 6u) {                // color dodge
        if (cb <= 0.0) { v = 0.0; }
        else if (cs >= 1.0) { v = 1.0; }
        else { v = min(cb / (1.0 - cs), 1.0); }
    } else if (mode == 7u) {                // color burn
        if (cb >= 1.0) { v = 1.0; }
        else if (cs <= 0.0) { v = 0.0; }
        else { v = 1.0 - min((1.0 - cb) / cs, 1.0); }
    } else if (mode == 8u) {                // hard light
        if (cs <= 0.5) { v = 2.0 * cs * cb; }
        else { v = 1.0 - 2.0 * (1.0 - cs) * (1.0 - cb); }
    } else if (mode == 9u) {                // soft light
        var d = sqrt(cb);
        if (cb <= 0.25) { d = ((16.0 * cb - 12.0) * cb + 4.0) * cb; }
        if (cs <= 0.5) { v = cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb); }
        else { v = cb + (2.0 * cs - 1.0) * (d - cb); }
    } else if (mode == 10u) {               // difference
        v = abs(cb - cs);
    } else if (mode == 11u) {               // exclusion
        v = cb + cs - 2.0 * cb * cs;
    }
    return clamp01(v);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let prev = textureSample(prev_tex, samp, in.uv);

    // Posizione in pixel del documento, poi nel layer tramite la matrice inversa.
    let doc_px = in.uv * u.doc_size;
    let layer_px = (u.inv * vec4<f32>(doc_px, 0.0, 1.0)).xy;

    var src = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var mask_value = 1.0;
    if (all(layer_px >= vec2<f32>(0.0)) && all(layer_px < u.layer_size)) {
        let uv = layer_px / u.layer_size;
        src = textureSample(layer_tex, samp, uv);
        // la maschera si campiona con le stesse UV del layer: dimensioni
        // diverse vengono adattate, esattamente come nel riferimento CPU
        mask_value = textureSample(mask_tex, samp, uv).r;
    }

    let ab = clamp01(prev.a);
    let av = clamp01(src.a) * u.opacity * mask_value;
    let ao = av + ab * (1.0 - av);
    if (ao <= 0.00001) {
        return vec4<f32>(0.0);
    }

    var out = vec3<f32>(0.0);
    for (var c = 0; c < 3; c = c + 1) {
        let cb = clamp01(prev[c]);
        let cs = clamp01(src[c]);
        let b = blend_channel(u.mode, cb, cs);
        let co = av * (1.0 - ab) * cs + av * ab * b + (1.0 - av) * ab * cb;
        out[c] = clamp01(co / ao);
    }
    return vec4<f32>(out, clamp01(ao));
}
