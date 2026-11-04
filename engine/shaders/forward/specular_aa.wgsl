// engine/shaders/forward/specular_aa.wgsl — geometric specular anti-aliasing (#1702):
// a GGX lobe narrower than the spread of normals one pixel covers aliases, so a small
// highlight on a curved or bumpy surface crawls and sparkles as the camera moves. This
// widens the lobe by that spread (Tokuyoshi & Kaplanyan 2019, "Improved Geometric
// Specular Antialiasing"), as Filament's `normalFiltering` and HDRP's
// `GeometricNormalFiltering` do.
//
// The constants are Filament's defaults (`specularAntiAliasingVariance` 0.15,
// `specularAntiAliasingThreshold` 0.2, applied as `min(2 * variance, threshold)`; HDRP
// takes 0.1 and squares its threshold). It is on for every lit surface, as Filament's
// is, and has no knob: a knob would serve a stylised material that wants a razor
// highlight on a dense mesh at the price of shimmer, which no game has asked for.

#define_import_path forward::specular_aa

// The screen-space variance's scale: how much of the normal's change across a pixel
// counts as spread inside it.
const SCREEN_SPACE_VARIANCE: f32 = 0.15;
// The most a pixel's footprint may add to α² (squared GGX roughness): a silhouette,
// where the normal swings across a pixel, would otherwise turn a mirror matte.
const THRESHOLD: f32 = 0.2;

// What one pixel's footprint adds to α²: the variance of the interpolated vertex normal
// `n` across the pixel, from its screen derivatives. Call it before any per-pixel
// branch (src/render/CLAUDE.md); `widen` then applies it anywhere. Normal-map detail is
// not in `n` (its mips would carry it, Toksvig: not done yet, #1702).
fn kernel(n: vec3<f32>) -> f32 {
    let du = dpdx(n);
    let dv = dpdy(n);
    let variance = SCREEN_SPACE_VARIANCE * (dot(du, du) + dot(dv, dv));
    return min(2.0 * variance, THRESHOLD);
}

// Perceptual roughness widened by `kernel`: α² + kernel, back to perceptual
// (`perceptual = α^½`). A flat surface (kernel 0) keeps its roughness, to rounding.
fn widen(perceptual: f32, kernel: f32) -> f32 {
    let alpha = perceptual * perceptual;
    return sqrt(sqrt(saturate(alpha * alpha + kernel)));
}
