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
// Two spreads count, each capped by `THRESHOLD`: the interpolated vertex normal's
// across the pixel (`kernel`), and a normal map's inside the texel it is read at
// (Toksvig 2005): a mip averages the normals it covers, and where they disagreed the
// average is shorter than 1, so a glossy floor with a busy normal map widens its
// highlight at a distance instead of sparkling. A map that keeps z (RGBA8, BC7) gives
// that length in its sample (`texture_kernel`); a BC5 map rebuilds z, so the importer
// measured each level's mean spread and the record carries it (`table_kernel`,
// `asset::texture_import::variance`). At the base level neither widens: a glossy floor
// stays glossy up close.

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

// `asset::texture_import::variance::QUANTUM`: the length 8-bit rounding can take from
// a whole normal, which is not spread.
const QUANTUM: f32 = 2.0 / 255.0;

// What a normal-map texel whose filtered normal is `length` long adds to α², at
// `level` (the level its footprint covers, `mapping::normal_level`): HDRP's
// `TextureNormalVariance` (κ = (3r − r³)/(1 − r²), variance 1/(4κ)), doubled and
// capped as its `TextureNormalFiltering` does, faded in over the first level, where
// trilinear filtering blends the base level (whole normals, only rounding and
// magnification shorten them) into level 1. The same sum as
// `asset::texture_import::variance::kernel`.
fn texture_kernel(length: f32, level: f32) -> f32 {
    let r = min(length + QUANTUM, 1.0);
    let r2 = r * r;
    let k = min(0.5 * (1.0 - r2) / max(r * (3.0 - r2), 1e-4), THRESHOLD * THRESHOLD);
    return k * saturate(level);
}

// What a normal map read at `level` adds to α², under its record's `use_normal_map`
// `flag`: from the sample's `length` where the map keeps z (1), from the record's
// `table` for BC5 (2).
fn map_kernel(flag: u32, table: vec4<u32>, length: f32, level: f32) -> f32 {
    if (flag == 2u) {
        return table_kernel(table, level);
    }
    return texture_kernel(length, level);
}

// What a BC5 normal map adds to α² at `level`, from its record's table (`VarianceTable`:
// level L's mean kernel in byte L − 1 of `table`, in 255ths of `THRESHOLD²`), levels
// between two entries blended as trilinear filtering blends them. Level 0 adds nothing.
fn table_kernel(table: vec4<u32>, level: f32) -> f32 {
    let l = clamp(level, 0.0, 16.0);
    let i = u32(floor(l));
    return mix(table_entry(table, i), table_entry(table, i + 1u), l - floor(l));
}

fn table_entry(table: vec4<u32>, level: u32) -> f32 {
    if (level == 0u) {
        return 0.0;
    }
    let j = min(level - 1u, 15u);
    let byte = (table[j / 4u] >> (8u * (j % 4u))) & 0xffu;
    return f32(byte) / 255.0 * THRESHOLD * THRESHOLD;
}
