
#define_import_path reactive

const MAX_REACTIVE: f32 = 0.9;

// The texel camera `camera` (its index in the stack) writes for reactivity `r`; `fixed` at or
// above 0 replaces it (a material's or an emitter's `Off` / `Full`), only where the draw laid
// down something (`r` at least a byte's step), so a `Full` sprite's transparent corners leave
// the pixel alone.
fn texel(r: f32, fixed: f32, camera: f32) -> vec4<f32> {
    // A NaN (a broken colour upstream) reads 0, never a garbage weight.
    let automatic = select(0.0, r, r >= 0.0);
    let value = select(automatic, fixed * step(1.0 / 255.0, automatic), fixed >= 0.0);
    let safe = min(value, MAX_REACTIVE);
    return vec4<f32>(select(0.0, camera + safe, safe > 0.0), 0.0, 0.0, 0.0);
}

fn over(alpha: f32) -> f32 {
    return alpha;
}

fn added(delta: vec3<f32>) -> f32 {
    let m = max(max(delta.r, delta.g), max(delta.b, 0.0));
    return m / (1.0 + m);
}

// A surface that multiplied the pixel by `tint`: it darkens by at most `1 - tint`.
fn multiplied(tint: vec3<f32>) -> f32 {
    let t = clamp(tint, vec3<f32>(0.0), vec3<f32>(1.0));
    return 1.0 - min(min(t.r, t.g), t.b);
}
