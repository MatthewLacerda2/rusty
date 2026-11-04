// engine/shaders/forward/specular_aa.wgsl — geometric specular anti-aliasing (#1702):
// a GGX lobe narrower than the spread of normals one pixel covers aliases, so a small
// highlight on a curved surface crawls and sparkles as the camera moves. This widens
// the lobe by that spread (Tokuyoshi & Kaplanyan 2019, "Improved Geometric Specular
// Antialiasing"), in HDRP's form (`CommonMaterial.hlsl`'s `GeometricNormalFiltering`),
// with HDRP Lit's default constants, since Unity is rusty's yardstick.
//
// It is on for every lit surface. HDRP and Filament both leave it off by default, as a
// per-material opt-in (HDRP's *Geometric Specular AA* toggle, Filament's
// `specularAntiAliasing`); rusty turns it on because the issue asks for it, it costs a
// few ALU ops, and HDRP's squared threshold keeps a glossy surface glossy. rusty has
// no per-material toggle yet: one would be a `MaterialAsset` bool read here.
//
// Only the interpolated vertex normal's spread counts. Normal-map detail does not, so
// a flat glossy floor with a busy normal map (the issue's own symptom) is not touched
// by this part: TAA, or Toksvig in the normal map's mips later, has it (#1702).

#define_import_path forward::specular_aa

// HDRP Lit's `_SpecularAAScreenSpaceVariance` default: how much of the normal's change
// across a pixel counts as spread inside it (at most 0.25, a Gaussian pixel of σ ½).
const SCREEN_SPACE_VARIANCE: f32 = 0.1;
// HDRP Lit's `_SpecularAAThreshold` default. HDRP squares it, so a pixel's footprint
// adds at most 0.04 to α² (squared GGX roughness): a mirror's α rises to at most 0.2
// (perceptual roughness 0.45) at a silhouette, where the normal swings across a pixel.
const THRESHOLD: f32 = 0.2;

// What one pixel's footprint adds to α²: the variance of the interpolated vertex normal
// `n` across the pixel, from its screen derivatives. Call it before any per-pixel
// branch (src/render/CLAUDE.md); `widen` then applies it anywhere.
fn kernel(n: vec3<f32>) -> f32 {
    let du = dpdx(n);
    let dv = dpdy(n);
    let variance = SCREEN_SPACE_VARIANCE * (dot(du, du) + dot(dv, dv));
    return min(2.0 * variance, THRESHOLD * THRESHOLD);
}

// Perceptual roughness widened by `kernel`: α² + kernel, back to perceptual
// (`perceptual = α^½`). A flat surface (kernel 0) keeps its roughness, to rounding.
fn widen(perceptual: f32, kernel: f32) -> f32 {
    let alpha = perceptual * perceptual;
    return sqrt(sqrt(saturate(alpha * alpha + kernel)));
}
