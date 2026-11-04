// engine/shaders/postfx/effects/tonemapping.wgsl — Tonemapping (#1440, #1701): the
// operator that brings scene-linear HDR to the display (slot: x its index, mirroring
// `Tonemap::to_index`: 0 none, 1 Reinhard, 2 ACES, 3 Neutral, 4 AgX), and, last in
// the grade, the display-gamma tweak (slot: x the gamma). Both are baked into the
// grading LUT, so a curve's cost is paid per texel of the table; per pixel every
// mode costs the same LUT read (`postfx::grade::lut_sample`, four texel loads).

#define_import_path postfx::effects::tonemapping

// Narkowicz's fit of the ACES RRT + sRGB ODT. The fit itself is pre-exposed (1 in
// maps to ~0.8 out); his note: "for the original ACES curve just multiply input by
// 0.6", which is the curve Unity's ACES mode draws (#1701).
fn aces(x_in: vec3<f32>) -> vec3<f32> {
    let x = x_in * 0.6;
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Khronos PBR Neutral (glTF's reference implementation): below 0.76 a colour passes
// through, less a small black offset; above it the peak channel rolls off towards 1
// and the colour moves towards white along a line, so its hue is kept.
fn pbr_neutral(c_in: vec3<f32>) -> vec3<f32> {
    let start = 0.8 - 0.04;
    let desaturation = 0.15;
    let low = min(c_in.r, min(c_in.g, c_in.b));
    let offset = select(0.04, low - 6.25 * low * low, low < 0.08);
    var c = c_in - offset;
    let peak = max(c.r, max(c.g, c.b));
    if (peak < start) {
        return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    let d = 1.0 - start;
    let new_peak = 1.0 - d * d / (peak + d - start);
    c *= new_peak / peak;
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return clamp(mix(c, vec3<f32>(new_peak), g), vec3<f32>(0.0), vec3<f32>(1.0));
}

// AgX, Troy Sobotka's, as Blender 4 ships it, by way of Filament and three.js: into
// Rec.2020, an inset gamut, a log2 encoding of [-12.47, 4.03] EV around middle grey
// (so the curve reaches white near 16, well inside the LUT's ~50), a polynomial
// sigmoid, the outset, and back. Matrices are column-major, as WGSL's constructor.
const SRGB_TO_REC2020 = mat3x3<f32>(
    vec3<f32>(0.6274, 0.0691, 0.0164),
    vec3<f32>(0.3293, 0.9195, 0.0880),
    vec3<f32>(0.0433, 0.0113, 0.8956),
);
const REC2020_TO_SRGB = mat3x3<f32>(
    vec3<f32>(1.6605, -0.1246, -0.0182),
    vec3<f32>(-0.5876, 1.1329, -0.1006),
    vec3<f32>(-0.0728, -0.0083, 1.1187),
);
const AGX_INSET = mat3x3<f32>(
    vec3<f32>(0.856627153315983, 0.137318972929847, 0.11189821299995),
    vec3<f32>(0.0951212405381588, 0.761241990602591, 0.0767994186031903),
    vec3<f32>(0.0482516061458583, 0.101439036467562, 0.811302368396859),
);
const AGX_OUTSET = mat3x3<f32>(
    vec3<f32>(1.1271005818144368, -0.1413297634984383, -0.14132976349843826),
    vec3<f32>(-0.11060664309660323, 1.157823702216272, -0.11060664309660294),
    vec3<f32>(-0.016493938717834573, -0.016493938717834257, 1.2519364065950405),
);
const AGX_MIN_EV: f32 = -12.47393;
const AGX_MAX_EV: f32 = 4.026069;

fn agx_sigmoid(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2
        + 0.1191 * x - 0.00232;
}

fn agx(c_in: vec3<f32>) -> vec3<f32> {
    var c = AGX_INSET * (SRGB_TO_REC2020 * c_in);
    c = log2(max(c, vec3<f32>(1e-10)));
    c = clamp((c - AGX_MIN_EV) / (AGX_MAX_EV - AGX_MIN_EV), vec3<f32>(0.0), vec3<f32>(1.0));
    c = AGX_OUTSET * agx_sigmoid(c);
    c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
    return clamp(REC2020_TO_SRGB * c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap_with(c: vec3<f32>, mode: u32) -> vec3<f32> {
    switch (mode) {
        case 1u: { return c / (c + vec3<f32>(1.0)); }
        case 2u: { return aces(c); }
        case 3u: { return pbr_neutral(c); }
        case 4u: { return agx(c); }
        default: { return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)); }
    }
}

fn tonemap(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    return tonemap_with(c, u32(p.x));
}

// The display-gamma tweak (neutral 1), NOT the output encode: every target the
// composite writes is sRGB, and the hardware encodes on write (#415).
fn display_gamma(c_in: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    let c = clamp(c_in, vec3<f32>(0.0), vec3<f32>(1.0));
    return pow(c, vec3<f32>(1.0 / max(p.x, 0.01)));
}
