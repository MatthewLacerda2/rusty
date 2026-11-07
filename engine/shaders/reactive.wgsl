// engine/shaders/reactive.wgsl — TAA's reactive mask (#1845): how much of a pixel's
// colour a translucent draw (a transparent surface, a sprite particle, a ribbon)
// laid down this frame, which the resolve (`postfx/taa.wgsl`) trusts over the
// history. A translucent writes no motion of its own, so without it the history
// reprojects it with the surface behind and averages its detail away.
//
// The formulas follow FSR2's. A surface blending over what is behind it writes its
// alpha (the FSR2 README: "a good proxy for reactiveness is the alpha value used
// when compositing"); one adding to it writes how much it changed the pixel, an
// approximation of FSR2's `GenerateReactiveMask` (the largest channel of the change,
// tonemapped by `x / (1 + x)`: FSR2 tonemaps the colour before and after
// compositing and takes the difference, but a pass here can't read what is under
// it, so only the added light is tonemapped); one multiplying it writes the most it
// can darken it. All clamp to FSR2's recommended maximum, 0.9.
//
// The target is `R16Float`, blended by max, holding one packed value: the drawing
// camera's index in its stack plus its reactivity (below 1), so a higher camera's
// write replaces a lower one's whatever their reactivities, and the resolve
// ignores a world camera's translucent under a viewmodel's opaque gun. A draw that
// lays down nothing (a sprite's transparent corner) writes 0, which the max
// leaves alone. The price: where a higher camera's translucent lies over a lower
// camera's surface, only the higher camera's reactivity is kept there.

#define_import_path reactive

// FSR2's recommended maximum reactivity (`components::MAX_REACTIVE`).
const MAX_REACTIVE: f32 = 0.9;

// The texel camera `camera` (its index in the stack) writes for reactivity `r`;
// `fixed` at or above 0 replaces it (a material's or an emitter's `Off` / `Full`),
// only where the draw laid down something (`r` at least a byte's step), so a
// `Full` sprite's transparent corners leave the pixel alone.
fn texel(r: f32, fixed: f32, camera: f32) -> vec4<f32> {
    // A NaN (a broken colour upstream) reads 0, never a garbage weight.
    let automatic = select(0.0, r, r >= 0.0);
    let value = select(automatic, fixed * step(1.0 / 255.0, automatic), fixed >= 0.0);
    let safe = min(value, MAX_REACTIVE);
    return vec4<f32>(select(0.0, camera + safe, safe > 0.0), 0.0, 0.0, 0.0);
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
