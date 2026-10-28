// engine/shaders/postfx/effects/color_adjustments.wgsl — Color Adjustments (#1440),
// all on the scene-linear colour before the tonemap, where Unity URP's grading LUT
// runs them (#1479): contrast in log space around mid-grey, right after white
// balance (slot: x contrast, y the tonemapper), the hue shift (slot: x the shift in
// turns), then saturation (slot: x saturation). Each leaves the colour exactly as it
// was at its neutral.

#define_import_path postfx::effects::color_adjustments

#import postfx::grade::luminance

fn rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1e-4;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + k.xyz) * 6.0 - k.www);
    return c.z * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn hue(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (p.x != 0.0) {
        let hsv = rgb_to_hsv(out);
        out = hsv_to_rgb(vec3<f32>(fract(hsv.x + p.x), hsv.yz));
    }
    return out;
}

// The log spaces URP's contrast works in (`LutBuilderHdr.shader`): ACEScc under the
// ACES tonemapper, ARRI ALEXA LogC (EI 800) under any other. Parameters and toes are
// URP's (`Color.hlsl`). URP converts to the ACES AP0 primaries before ACEScc; rusty's
// ACES is a fit on linear sRGB, so the colour stays in sRGB primaries here.
const LOGC_CUT: f32 = 0.011361;
const LOGC_A: f32 = 5.555556;
const LOGC_B: f32 = 0.047996;
const LOGC_C: f32 = 0.244161;
const LOGC_D: f32 = 0.386036;
const LOGC_E: f32 = 5.301883;
const LOGC_F: f32 = 0.092819;

fn linear_to_logc(x: vec3<f32>) -> vec3<f32> {
    let log = LOGC_C * log2(LOGC_A * x + LOGC_B) / log2(10.0) + LOGC_D;
    return select(LOGC_E * x + LOGC_F, log, x > vec3<f32>(LOGC_CUT));
}

fn logc_to_linear(t: vec3<f32>) -> vec3<f32> {
    let lin = (pow(vec3<f32>(10.0), (t - LOGC_D) / LOGC_C) - LOGC_B) / LOGC_A;
    return select((t - LOGC_F) / LOGC_E, lin, t > vec3<f32>(LOGC_E * LOGC_CUT + LOGC_F));
}

// ACEScc: log2 above 2^-15, a toe below it that meets zero at -0.3584.
const ACESCC_TOE: f32 = 3.0517578e-5; // 2^-15
const HALF_MAX: f32 = 65504.0;

fn linear_to_acescc(x: vec3<f32>) -> vec3<f32> {
    let toe = (log2(exp2(-16.0) + x * 0.5) + 9.72) / 17.52;
    let log = (log2(max(x, vec3<f32>(ACESCC_TOE))) + 9.72) / 17.52;
    return select(toe, log, x >= vec3<f32>(ACESCC_TOE));
}

fn acescc_to_linear(t: vec3<f32>) -> vec3<f32> {
    let lin = exp2(t * 17.52 - 9.72);
    let toe = (lin - exp2(-16.0)) * 2.0;
    return select(toe, min(lin, vec3<f32>(HALF_MAX)), t >= vec3<f32>((9.72 - 15.0) / 17.52));
}

// URP's pivot, ACEScc's code for the 0.18 grey card. Under ACES that is exactly
// linear 0.18; in LogC, where URP applies the same code, it sits near 0.22.
const MIDGREY: f32 = 0.4135884;
// `Tonemap::Aces`'s index, as `tonemapping.wgsl` reads it.
const TONEMAP_ACES: u32 = 2u;

// Steepens (`> 1`) or flattens the curve around mid-grey in log space (slot: x the
// contrast, y the tonemapper's index), so a moderate boost darkens the shadows
// without clipping them to black, the way URP's does. Negative results clamp to
// black, as URP's do.
fn contrast(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    if (p.x == 1.0) {
        return c;
    }
    let grey = vec3<f32>(MIDGREY);
    let x = max(c, vec3<f32>(0.0));
    var out: vec3<f32>;
    if (u32(p.y) == TONEMAP_ACES) {
        out = acescc_to_linear((linear_to_acescc(x) - grey) * p.x + grey);
    } else {
        out = logc_to_linear((linear_to_logc(x) - grey) * p.x + grey);
    }
    return max(out, vec3<f32>(0.0));
}

fn saturation(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    if (p.x == 1.0) {
        return c;
    }
    return max(mix(vec3<f32>(luminance(c)), c, p.x), vec3<f32>(0.0));
}
