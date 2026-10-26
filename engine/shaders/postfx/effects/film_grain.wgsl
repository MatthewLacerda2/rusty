// engine/shaders/postfx/effects/film_grain.wgsl — Film Grain (#1439), Unity URP's
// response curve, the last thing the composite draws. Seeded by the pixel and the game
// clock: new noise every tick while time runs, the same noise at the same tick (and
// frozen at time 0, in edit mode). Slot 0: x intensity (`0`: off), y response.

#define_import_path postfx::effects::film_grain

#import postfx::frame::{Frag, params}
#import postfx::grade::luminance

// A hash of the pixel and the game clock in [-1, 1].
fn noise(px: vec2<u32>, time: f32) -> f32 {
    var h = (px.x * 73856093u) ^ (px.y * 19349663u) ^ (bitcast<u32>(time) * 83492791u);
    h = (h ^ (h >> 16u)) * 2246822519u;
    h = (h ^ (h >> 13u)) * 3266489917u;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967295.0 * 2.0 - 1.0;
}

// The noise scales the colour, and fades out in the highlights as `response` rises.
fn film_grain(c: vec3<f32>, px: vec2<u32>, p: vec4<f32>) -> vec3<f32> {
    let n = noise(px, params.camera_pos.w);
    let lum = mix(1.0, 1.0 - sqrt(luminance(saturate(c))), p.y);
    return max(c + c * n * p.x * lum, vec3<f32>(0.0));
}

fn display(c: vec3<f32>, f: Frag, p: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (p.x > 0.0) {
        out = film_grain(out, vec2<u32>(f.pos), p);
    }
    return out;
}
