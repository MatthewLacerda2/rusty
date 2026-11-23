// The scene fog's height profile and its modes live here; `common::fog_over` is their one
// caller.

#define_import_path fog::height

// Mean relative fog thickness along a ray whose heights run `a`..`b` above the base
// height: full (1) below it, `e^(-k·h)` above it. Exact integral of that profile,
// so a ray that dips under the base picks up the full layer for that stretch.
fn fog_height_mean(k: f32, a: f32, b: f32) -> f32 {
    let lo = min(a, b);
    let hi = max(a, b);
    if (k <= 0.0) {
        return 1.0;
    }
    if (hi - lo < 1e-3) {
        return exp(-k * max(lo, 0.0));
    }
    let below = min(hi, 0.0) - min(lo, 0.0);
    let above = (exp(-k * max(lo, 0.0)) - exp(-k * max(hi, 0.0))) / k;
    return (below + above) / (hi - lo);
}

// `mode` (0 off, 1 linear, 2 exponential, 3 exponential²) over `length` metres whose mean
// thickness is `h`; `range` is (start, end, density).
fn fog_thickness(mode: u32, range: vec3<f32>, length: f32, h: f32) -> f32 {
    if (mode == 0u) {
        return 0.0;
    }
    let d = max(length - range.x, 0.0);
    var f: f32;
    if (mode == 1u) {
        f = d * h / max(range.y - range.x, 1e-3);
    } else if (mode == 2u) {
        f = 1.0 - exp(-range.z * d * h);
    } else {
        let x = range.z * d * h;
        f = 1.0 - exp(-x * x);
    }
    return clamp(f, 0.0, 1.0);
}
