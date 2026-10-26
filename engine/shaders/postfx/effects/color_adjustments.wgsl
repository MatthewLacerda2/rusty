// engine/shaders/postfx/effects/color_adjustments.wgsl — Color Adjustments (#1440):
// the hue shift on the scene-linear colour before the tonemap (slot: x the shift in
// turns), contrast and saturation on the tonemapped colour after it (slot: x
// contrast, y saturation).

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

fn contrast_saturation(c_in: vec3<f32>, p: vec4<f32>) -> vec3<f32> {
    var c = c_in;
    c = (c - vec3<f32>(0.5)) * p.x + vec3<f32>(0.5);
    c = mix(vec3<f32>(luminance(c)), c, p.y);
    return c;
}
