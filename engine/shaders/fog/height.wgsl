// engine/shaders/fog/height.wgsl — the height profile of the scene's fog (#437):
// `common::fog_factor` is its one caller.

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
