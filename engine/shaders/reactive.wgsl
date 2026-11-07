// engine/shaders/reactive.wgsl — TAA's reactive mask (#1845): how much of a pixel's
// colour a translucent draw (a transparent surface, a sprite particle, a ribbon)
// laid down this frame, which the resolve (`postfx/taa.wgsl`) trusts over the
// history. A translucent writes no motion of its own, so without it the history
// reprojects it with the surface behind and averages its detail away.
//
// The formulas are FSR2's. A surface blending over what is behind it writes its
// alpha (the FSR2 README: "a good proxy for reactiveness is the alpha value used
// when compositing"); one adding to it writes how much it changed the pixel, as
// FSR2's `GenerateReactiveMask` measures it (the largest channel of the change,
// tonemapped by `x / (1 + x)`); one multiplying it writes the most it can darken
// it. All clamp to FSR2's recommended maximum, 0.9.
//
// The target is `Rg8Unorm`, blended by max: r the reactivity, g which camera of the
// stack wrote it, `(index + 1) / 255`, so the resolve ignores a world camera's
// translucent under a viewmodel's opaque gun.

#define_import_path reactive

// FSR2's recommended maximum reactivity (`components::MAX_REACTIVE`).
const MAX_REACTIVE: f32 = 0.9;

// The texel camera `camera` (its index in the stack) writes for reactivity `r`;
// `fixed` at or above 0 replaces it (a material's or an emitter's `Off` / `Full`).
fn texel(r: f32, fixed: f32, camera: f32) -> vec4<f32> {
    let value = select(r, fixed, fixed >= 0.0);
    // A NaN (a broken colour upstream) reads 0, never a garbage weight.
    let safe = select(0.0, min(value, MAX_REACTIVE), value >= 0.0);
    return vec4<f32>(safe, (camera + 1.0) / 255.0, 0.0, 0.0);
}

// A surface blended over the pixel by `alpha`.
fn over(alpha: f32) -> f32 {
    return alpha;
}

// A surface that added `delta` to the pixel.
fn added(delta: vec3<f32>) -> f32 {
    let m = max(max(delta.r, delta.g), max(delta.b, 0.0));
    return m / (1.0 + m);
}

// A surface that multiplied the pixel by `tint`: it darkens by at most `1 - tint`.
fn multiplied(tint: vec3<f32>) -> f32 {
    let t = clamp(tint, vec3<f32>(0.0), vec3<f32>(1.0));
    return 1.0 - min(min(t.r, t.g), t.b);
}
