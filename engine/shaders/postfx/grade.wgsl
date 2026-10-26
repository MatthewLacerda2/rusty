// engine/shaders/postfx/grade.wgsl — the colour grade and the tonemapper (#1440),
// and where an HDR colour sits in the grading LUT they are baked into.
//
// `lut_bake.wgsl` evaluates `grade` once per texel of a 32³ table whenever a grading
// knob or the tonemap changes; the composite (`postfx.wgsl`) then grades every pixel
// with one fetch at `lut_coord`. The table is indexed by a log encoding (`encode`),
// so its 32 steps per channel span HDR from 0 to about 50, densest in the darks
// where the eye needs them. Unity URP and Unreal build their HDR grading LUT the
// same way.

#define_import_path postfx::grade

// Texels per side of the LUT. Mirrors `GradeLut::SIZE`.
const LUT_SIZE: f32 = 32.0;

// Mirrors the Rust `GradeParams`.
struct Grade {
    // xyz: LMS white-point coefficients, w: 1 when white balance runs
    white_balance: vec4<f32>,
    // xyz: lift, added after gain
    lift: vec4<f32>,
    // xyz: 1 / gamma, the gamma wheel's per-channel power
    inv_gamma: vec4<f32>,
    // xyz: gain, the per-channel scale
    gain: vec4<f32>,
    // x: hue shift in turns
    hue: vec4<f32>,
    // x: tonemap index (0 none, 1 Reinhard, 2 ACES), y: contrast, z: saturation,
    // w: display gamma
    tone: vec4<f32>,
};

// The table's log encoding: t = log2(1 + 64x) / LOG_SPAN, a LogC-shaped curve
// (linear-ish in the darks, logarithmic above) whose span is chosen so black
// (t = 0) and display white (t = 16/31) fall exactly on texels. Unity's LogC puts
// neither on one, and interpolating across a texel there lifts black by a level and
// pulls an untonemapped 1.0 down by seven. The table reaches x ≈ 50.8 (t = 1);
// brighter colour reads the top texel.
const LOG_SCALE: f32 = 64.0;
// log2(1 + 64) · 31 / 16: places x = 1 on texel 16 of 0..31.
const LOG_SPAN: f32 = 11.668338;

fn encode(x: vec3<f32>) -> vec3<f32> {
    return log2(1.0 + LOG_SCALE * max(x, vec3<f32>(0.0))) / LOG_SPAN;
}

fn decode(t: vec3<f32>) -> vec3<f32> {
    return (exp2(t * LOG_SPAN) - 1.0) / LOG_SCALE;
}

// The LUT coordinate of HDR colour `c`: its log code, mapped onto texel centres.
fn lut_coord(c: vec3<f32>) -> vec3<f32> {
    let t = clamp(encode(c), vec3<f32>(0.0), vec3<f32>(1.0));
    return t * ((LUT_SIZE - 1.0) / LUT_SIZE) + 0.5 / LUT_SIZE;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap(c: vec3<f32>, mode: u32) -> vec3<f32> {
    if (mode == 1u) {
        return c / (c + vec3<f32>(1.0));
    } else if (mode == 2u) {
        return aces(c);
    }
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Unity's white balance: scale the colour in LMS cone space by the coefficients
// that move D65 to the chosen white point. `v * M` with M's columns the rows of
// Unity's matrices is its `mul(M, v)`.
fn white_balance(c: vec3<f32>, coeffs: vec3<f32>) -> vec3<f32> {
    let to_lms = mat3x3<f32>(
        vec3<f32>(3.90405e-1, 5.49941e-1, 8.92632e-3),
        vec3<f32>(7.08416e-2, 9.63172e-1, 1.35775e-3),
        vec3<f32>(2.31082e-2, 1.28021e-1, 9.36245e-1),
    );
    let from_lms = mat3x3<f32>(
        vec3<f32>(2.85847e+0, -1.62879e+0, -2.48910e-2),
        vec3<f32>(-2.10182e-1, 1.15820e+0, 3.24281e-4),
        vec3<f32>(-4.18120e-2, -1.18169e-1, 1.06867e+0),
    );
    return ((c * to_lms) * coeffs) * from_lms;
}

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

// The whole grade of scene-linear HDR colour `c` (after exposure) to the display
// colour the composite writes. Unity's order for what it shares: white balance,
// Lift / Gamma / Gain and the hue shift run in scene-linear HDR, before the
// tonemap; contrast, saturation and the display-gamma tweak run after it, on the
// tonemapped colour, as the composite applied them before the LUT existed.
fn grade(c_in: vec3<f32>, g: Grade) -> vec3<f32> {
    var c = c_in;
    if (g.white_balance.w > 0.5) {
        c = white_balance(c, g.white_balance.xyz);
    }
    c = c * g.gain.xyz + g.lift.xyz;
    c = sign(c) * pow(abs(c), g.inv_gamma.xyz);
    if (g.hue.x != 0.0) {
        let hsv = rgb_to_hsv(max(c, vec3<f32>(0.0)));
        c = hsv_to_rgb(vec3<f32>(fract(hsv.x + g.hue.x), hsv.yz));
    }
    c = tonemap(c, u32(g.tone.x));
    c = (c - vec3<f32>(0.5)) * g.tone.y + vec3<f32>(0.5);
    c = mix(vec3<f32>(luminance(c)), c, g.tone.z);
    // The display-gamma tweak (neutral 1), NOT the output encode: every target the
    // composite writes is sRGB, and the hardware encodes on write (#415).
    c = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return pow(c, vec3<f32>(1.0 / max(g.tone.w, 0.01)));
}

// The grade of LUT texel `id`, the colour its log code decodes to.
fn grade_texel(id: vec3<u32>, g: Grade) -> vec3<f32> {
    return grade(decode(vec3<f32>(id) / (LUT_SIZE - 1.0)), g);
}
