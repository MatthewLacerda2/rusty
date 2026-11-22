
#define_import_path forward::specular_aa

// HDRP Lit's `_SpecularAAScreenSpaceVariance` default: how much of the normal's change across a
// pixel counts as spread inside it (at most 0.25, a Gaussian pixel of σ ½).
const SCREEN_SPACE_VARIANCE: f32 = 0.1;
// HDRP Lit's `_SpecularAAThreshold` default. Squared, it is the most each spread adds to α²
// (`variance::KERNEL_MAX`).
const THRESHOLD: f32 = 0.2;

// Takes derivatives: call it before any per-pixel branch.
fn kernel(n: vec3<f32>) -> f32 {
    let du = dpdx(n);
    let dv = dpdy(n);
    let variance = SCREEN_SPACE_VARIANCE * (dot(du, du) + dot(dv, dv));
    return min(2.0 * variance, THRESHOLD * THRESHOLD);
}

fn widen(perceptual: f32, kernel: f32) -> f32 {
    let alpha = perceptual * perceptual;
    return sqrt(sqrt(saturate(alpha * alpha + kernel)));
}

// `asset::texture_import::variance::QUANTUM`: the length 8-bit rounding can take from a whole
// normal, which is not spread.
const QUANTUM: f32 = 2.0 / 255.0;

// What a normal-map texel whose filtered normal is `length` long adds to α², at `level` (the
// level its footprint covers, `mapping::normal_level`): HDRP's `TextureNormalVariance` (κ = (3r
// − r³)/(1 − r²), variance 1/(4κ)), doubled and capped as its `TextureNormalFiltering` does,
// faded in over the first level, where trilinear filtering blends the base level (whole
// normals, only rounding and magnification shorten them) into level 1. The same sum as
// `asset::texture_import::variance::kernel`: change both together.
fn texture_kernel(length: f32, level: f32) -> f32 {
    let r = min(length + QUANTUM, 1.0);
    let r2 = r * r;
    let k = min(0.5 * (1.0 - r2) / max(r * (3.0 - r2), 1e-4), THRESHOLD * THRESHOLD);
    return k * saturate(level);
}

// What a normal map read at `level` adds to α²: from the record's `table` for BC5, else from the
// sample's `length`.
fn map_kernel(flag: u32, table: vec4<u32>, length: f32, level: f32) -> f32 {
    if (flag == 2u) {
        return table_kernel(table, level);
    }
    return texture_kernel(length, level);
}

// What a BC5 normal map adds to α² at `level`, from its record's table (`VarianceTable`: level
// L's mean kernel in byte L − 1 of `table`, in 255ths of `THRESHOLD²`), levels between two
// entries blended as trilinear filtering blends them.
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
