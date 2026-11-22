
#define_import_path postfx::grade

const LUT_SIZE: f32 = 32.0;

// The table's log encoding: t = log2(1 + 64x) / LOG_SPAN, a LogC-shaped curve (linear-ish in
// the darks, logarithmic above) whose span is chosen so black (t = 0) and display white (t =
// 16/31) fall exactly on texels.
const LOG_SCALE: f32 = 64.0;
// log2(1 + 64) · 31 / 16: places x = 1 on texel 16 of 0..31.
const LOG_SPAN: f32 = 11.668338;

fn encode(x: vec3<f32>) -> vec3<f32> {
    return log2(1.0 + LOG_SCALE * max(x, vec3<f32>(0.0))) / LOG_SPAN;
}

fn decode(t: vec3<f32>) -> vec3<f32> {
    return (exp2(t * LOG_SPAN) - 1.0) / LOG_SCALE;
}

fn lut_sample(lut: texture_3d<f32>, c: vec3<f32>) -> vec3<f32> {
    let p = clamp(encode(c), vec3<f32>(0.0), vec3<f32>(1.0)) * (LUT_SIZE - 1.0);
    let base = min(floor(p), vec3<f32>(LUT_SIZE - 2.0));
    let f = p - base;
    var a = vec3<i32>(1, 0, 0);
    var b = vec3<i32>(0, 1, 0);
    var w = f;
    if (f.x >= f.y) {
        if (f.y >= f.z) {
            w = f.xyz;
        } else if (f.x >= f.z) {
            b = vec3<i32>(0, 0, 1);
            w = f.xzy;
        } else {
            a = vec3<i32>(0, 0, 1);
            b = vec3<i32>(1, 0, 0);
            w = f.zxy;
        }
    } else if (f.z >= f.y) {
        a = vec3<i32>(0, 0, 1);
        w = f.zyx;
    } else if (f.z >= f.x) {
        a = vec3<i32>(0, 1, 0);
        b = vec3<i32>(0, 0, 1);
        w = f.yzx;
    } else {
        a = vec3<i32>(0, 1, 0);
        b = vec3<i32>(1, 0, 0);
        w = f.yxz;
    }
    let i = vec3<i32>(base);
    let c0 = textureLoad(lut, i, 0).rgb;
    let c1 = textureLoad(lut, i + a, 0).rgb;
    let c2 = textureLoad(lut, i + a + b, 0).rgb;
    let c3 = textureLoad(lut, i + vec3<i32>(1), 0).rgb;
    return (1.0 - w.x) * c0 + (w.x - w.y) * c1 + (w.y - w.z) * c2 + w.z * c3;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
