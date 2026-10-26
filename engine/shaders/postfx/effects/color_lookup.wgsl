// engine/shaders/postfx/effects/color_lookup.wgsl — the Color Lookup (#1441), the last
// step of the grading bake: the user LUT, as URP applies it, on the display colour,
// sRGB-encoded, mapped through the LUT's domain and blended in by its contribution.
// Slot 0: xyz the LUT's input range start, w its contribution (`0`: none applies);
// slot 1: xyz the range end.

#define_import_path postfx::effects::color_lookup

#import color::srgb::{decode_srgb, encode_srgb}

// The user LUT (`Rgba32Float`, red fastest), or a 1³ stand-in while none applies.
@group(0) @binding(2) var user_lut: texture_3d<f32>;

// Filtered here (trilinear, from exact `textureLoad`s) so the table needs no
// filterable format.
fn lookup(c: vec3<f32>, lo: vec4<f32>, hi: vec4<f32>) -> vec3<f32> {
    let s = encode_srgb(c);
    let n = textureDimensions(user_lut).x;
    let span = hi.xyz - lo.xyz;
    let t = clamp((s - lo.xyz) / span, vec3<f32>(0.0), vec3<f32>(1.0));
    let x = t * f32(n - 1u);
    let i = min(vec3<u32>(floor(x)), vec3<u32>(n - 2u));
    let f = x - vec3<f32>(i);
    var acc = vec3<f32>(0.0);
    for (var k = 0u; k < 8u; k++) {
        let o = vec3<u32>(k & 1u, (k >> 1u) & 1u, k >> 2u);
        let w = select(1.0 - f, f, o == vec3<u32>(1u));
        acc += textureLoad(user_lut, i + o, 0).rgb * (w.x * w.y * w.z);
    }
    let out = mix(s, acc, lo.w);
    return decode_srgb(clamp(out, vec3<f32>(0.0), vec3<f32>(1.0)));
}

fn grade(c: vec3<f32>, lo: vec4<f32>, hi: vec4<f32>) -> vec3<f32> {
    var out = c;
    if (lo.w > 0.0) {
        out = lookup(out, lo, hi);
    }
    return out;
}
