// engine/shaders/postfx/effects/chromatic_aberration.wgsl — Chromatic Aberration
// (#1439), Unity URP's uber-pass three-tap path, on the HDR image before the grade.
// Slot 0: x intensity (`0`: off, the image byte-identical).

#define_import_path postfx::effects::chromatic_aberration

#import postfx::frame::{Frag, t_color, s_color}

// The fringe at `uv`: red is `base`, the centre tap the composite already read, and
// green and blue are fetched toward the screen centre along the radius, so each
// channel shows the image a little further out than the last. What comes back is
// their *difference* from `base`, added to the running colour. The split grows with
// the squared distance from the centre, so the centre stays clean.
fn fringe(uv: vec2<f32>, intensity: f32, base: vec3<f32>) -> vec3<f32> {
    let coords = 2.0 * uv - 1.0;
    let end = uv - coords * dot(coords, coords) * intensity * 0.05;
    let delta = (end - uv) / 3.0;
    let g = textureSampleLevel(t_color, s_color, uv + delta, 0.0).g;
    let b = textureSampleLevel(t_color, s_color, uv + delta * 2.0, 0.0).b;
    return vec3<f32>(0.0, g - base.g, b - base.b);
}

fn hdr(c: vec3<f32>, f: Frag, p: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (p.x > 0.0) {
        out += fringe(f.uv, p.x, c);
    }
    return out;
}
