// engine/shaders/postfx/grade.wgsl — where an HDR colour sits in the grading LUT
// (#1440).
//
// The bake (generated, `render::postfx::effects::bake`) runs the grading effects'
// functions (`postfx/effects/*.wgsl`) once per texel of a 32³ table whenever a grading
// knob or the tonemap changes; the composite then grades every pixel with one fetch at
// `lut_coord`. The table is indexed by a log encoding (`encode`),
// so its 32 steps per channel span HDR from 0 to about 50, densest in the darks
// where the eye needs them. Unity URP and Unreal build their HDR grading LUT the
// same way.

#define_import_path postfx::grade

// Texels per side of the LUT. Mirrors `GradeLut::SIZE`.
const LUT_SIZE: f32 = 32.0;

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

// Rec. 709 luminance. Mirrors `core::color::luminance`, the weights Unity uses.
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
