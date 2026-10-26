// engine/shaders/color/srgb.wgsl — the sRGB transfer curves (IEC 61966-2-1), per
// channel: the UI's sampled-texture and fog conversions, and the user LUT's
// sRGB-encoded lookup in the grading bake (#1441).

#define_import_path color::srgb

// sRGB → linear.
fn decode_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((max(c, vec3<f32>(0.0)) + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// Linear → sRGB.
fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}
