// engine/shaders/postfx/lens.wgsl — the lens and film effects of the composite
// (#1439): chromatic aberration, vignette and film grain, with Unity URP's uber-pass
// formulas. `postfx.wgsl`'s `fs_composite` adds the aberration's fringe to the HDR
// colour before the grade, then runs `after_grade` on the graded, display-referred
// colour. Each effect is skipped outright at a zero intensity (what
// `render::postfx::params::pack_lens` packs for an effect that doesn't run), so an
// effect turned off leaves the image byte-identical.

#define_import_path postfx::lens

#import postfx::params::PostParams
#import postfx::grade::luminance

// The chromatic aberration's fringe at `uv` (Unity URP's three-tap path): red is
// `base`, the centre tap the composite already read, and green and blue are fetched
// toward the screen centre along the radius, so each channel shows the image a
// little further out than the last. What comes back is their *difference* from
// `base`, added to the running colour. The split grows with the squared distance
// from the centre, so the centre stays clean.
fn fringe(t: texture_2d<f32>, s: sampler, uv: vec2<f32>, intensity: f32, base: vec3<f32>) -> vec3<f32> {
    let coords = 2.0 * uv - 1.0;
    let end = uv - coords * dot(coords, coords) * intensity * 0.05;
    let delta = (end - uv) / 3.0;
    let g = textureSampleLevel(t, s, uv + delta, 0.0).g;
    let b = textureSampleLevel(t, s, uv + delta * 2.0, 0.0).b;
    return vec3<f32>(0.0, g - base.g, b - base.b);
}

// The vignette (Unity URP's, unrounded): the colour fades towards `color` with the
// distance from `center`, softer with a larger `smoothness`.
fn vignette(c: vec3<f32>, uv: vec2<f32>, p: PostParams) -> vec3<f32> {
    let d = abs(uv - p.vignette.zw) * p.vignette.x * 3.0;
    let falloff = pow(saturate(1.0 - dot(d, d)), p.vignette.y * 5.0);
    return c * mix(p.vignette_color.rgb, vec3<f32>(1.0), falloff);
}

// A hash of the pixel and the game clock in [-1, 1]: new noise every tick while
// time runs, the same noise at the same tick (and frozen at time 0, in edit mode).
fn grain_noise(px: vec2<u32>, time: f32) -> f32 {
    var h = (px.x * 73856093u) ^ (px.y * 19349663u) ^ (bitcast<u32>(time) * 83492791u);
    h = (h ^ (h >> 16u)) * 2246822519u;
    h = (h ^ (h >> 13u)) * 3266489917u;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967295.0 * 2.0 - 1.0;
}

// Film grain (Unity URP's response curve): the noise scales the colour, and fades
// out in the highlights as `response` rises.
fn film_grain(c: vec3<f32>, px: vec2<u32>, p: PostParams) -> vec3<f32> {
    let n = grain_noise(px, p.camera_pos.w);
    let lum = mix(1.0, 1.0 - sqrt(luminance(saturate(c))), p.lens.z);
    return max(c + c * n * p.lens.y * lum, vec3<f32>(0.0));
}

// The effects after the grade, in Unity's order: the vignette, then film grain
// last. `pos` is the fragment's pixel position (the grain's seed).
fn after_grade(c: vec3<f32>, uv: vec2<f32>, pos: vec2<f32>, p: PostParams) -> vec3<f32> {
    var out = c;
    if (p.vignette.x > 0.0) {
        out = vignette(out, uv, p);
    }
    if (p.lens.y > 0.0) {
        out = film_grain(out, vec2<u32>(pos), p);
    }
    return out;
}
