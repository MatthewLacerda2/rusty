// engine/shaders/postfx/lut_bake.wgsl — bakes the grading LUT (#1440): one
// invocation per texel of the 32³ table, each storing `grade` of the HDR colour the
// texel's log code decodes to (`grade::decode`, log2(1 + 64x) scaled so black and
// display white fall on texels), then the volume's user LUT over it (#1441). Runs
// only when a grading knob, the tonemap or the user LUT changes (`GradeLut::prepare`).

#import postfx::grade::{Grade, LUT_SIZE, grade_texel}
#import color::srgb::{decode_srgb, encode_srgb}

@group(0) @binding(0) var<uniform> g: Grade;
@group(0) @binding(1) var lut: texture_storage_3d<rgba16float, write>;
// The user LUT (`Rgba32Float`, red fastest), or a 1³ stand-in while
// `g.lookup_min.w` (its contribution) is 0.
@group(0) @binding(2) var user_lut: texture_3d<f32>;

// Unity's Color Lookup, as URP applies it: on the display colour, sRGB-encoded,
// mapped through the LUT's domain and blended in by its contribution. Filtered here
// (trilinear, from exact `textureLoad`s) so the table needs no filterable format.
fn color_lookup(c: vec3<f32>) -> vec3<f32> {
    let s = encode_srgb(c);
    let n = textureDimensions(user_lut).x;
    let span = g.lookup_max.xyz - g.lookup_min.xyz;
    let t = clamp((s - g.lookup_min.xyz) / span, vec3<f32>(0.0), vec3<f32>(1.0));
    let x = t * f32(n - 1u);
    let i = min(vec3<u32>(floor(x)), vec3<u32>(n - 2u));
    let f = x - vec3<f32>(i);
    var acc = vec3<f32>(0.0);
    for (var k = 0u; k < 8u; k++) {
        let o = vec3<u32>(k & 1u, (k >> 1u) & 1u, k >> 2u);
        let w = select(1.0 - f, f, o == vec3<u32>(1u));
        acc += textureLoad(user_lut, i + o, 0).rgb * (w.x * w.y * w.z);
    }
    let out = mix(s, acc, g.lookup_min.w);
    return decode_srgb(clamp(out, vec3<f32>(0.0), vec3<f32>(1.0)));
}

@compute @workgroup_size(4, 4, 4)
fn cs_bake(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id >= vec3<u32>(u32(LUT_SIZE)))) {
        return;
    }
    var c = grade_texel(id, g);
    if (g.lookup_min.w > 0.0) {
        c = color_lookup(c);
    }
    textureStore(lut, id, vec4<f32>(c, 1.0));
}
