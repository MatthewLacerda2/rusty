// engine/shaders/sky/noise.wgsl — the value noise the procedural sky's clouds are
// carved from (#256), as fractal Brownian motion, and how far the clouds' coverage
// shifts it: `common::procedural_sky` is their one caller.

#define_import_path sky::noise

fn sky_hash(p: vec2<f32>) -> f32 {
    // Deterministic 2D -> 1D hash in [0, 1). No engine RNG: this is the platform
    // (render) layer, and the result is a pure function of direction.
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn sky_value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Smoothstep interpolation weights for C1-continuous cells.
    let u = f * f * (3.0 - 2.0 * f);
    let a = sky_hash(i + vec2<f32>(0.0, 0.0));
    let b = sky_hash(i + vec2<f32>(1.0, 0.0));
    let c = sky_hash(i + vec2<f32>(0.0, 1.0));
    let d = sky_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn sky_fbm(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = p;
    for (var i = 0; i < 5; i = i + 1) {
        sum = sum + amp * sky_value_noise(freq);
        freq = freq * 2.0;
        amp = amp * 0.5;
    }
    return sum;
}

// How far `coverage` shifts the FBM before the cloud mask's ramp (0.45..0.80) carves
// it: exactly 0 at the default 0.5 (the layer the sky always drew), -0.55 at 0 so
// even the FBM's maximum (31/32) stays under the ramp and nothing is carved, +0.8 at
// 1 so even its minimum (0) clears it and every direction is cloud.
fn cloud_shift(coverage: f32) -> f32 {
    let c = coverage - 0.5;
    return c * select(1.1, 1.6, c > 0.0);
}
