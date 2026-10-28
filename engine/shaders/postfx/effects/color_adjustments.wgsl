// engine/shaders/postfx/effects/color_adjustments.wgsl — Color Adjustments (#1440),
// all on the scene-linear colour before the tonemap, where Unity URP's grading LUT
// runs them (#1479): contrast in LogC around mid-grey, right after white balance
// (slot: x contrast), the hue shift (slot: x the shift in turns), then saturation
// (slot: x saturation). Each leaves the colour exactly as it was at its neutral.

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

// ARRI ALEXA LogC (EI 800), the log space URP's contrast works in. Its parameters
// are URP's `LogC` (`Color.hlsl`).
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

// URP's pivot: ACEScc's mid-grey code, applied in LogC (where it sits at linear
// ~0.22, a little above the 0.18 grey card). Kept as URP has it, so a contrast
// authored against Unity grades the same here.
const MIDGREY: f32 = 0.4135884;

// Steepens (`> 1`) or flattens the curve around mid-grey in LogC, so a moderate
// boost darkens the shadows without clipping them to black, the way URP's does.
// Negative results clamp to black, as URP's do.
fn contrast(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    if (p.x == 1.0) {
        return c;
    }
    let grey = vec3<f32>(MIDGREY);
    let log = (linear_to_logc(max(c, vec3<f32>(0.0))) - grey) * p.x + grey;
    return max(logc_to_linear(log), vec3<f32>(0.0));
}

fn saturation(c: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    if (p.x == 1.0) {
        return c;
    }
    return max(mix(vec3<f32>(luminance(c)), c, p.x), vec3<f32>(0.0));
}
