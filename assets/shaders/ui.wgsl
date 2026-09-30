// ui.wgsl — the in-game UI pass (#418).
//
// Draws laid-out UI graphics onto the finished frame, after post-FX. Vertices
// arrive in NDC with a straight-alpha tint in display (sRGB-encoded) space. The
// pass targets a NON-sRGB view of the frame, so blending happens on the encoded
// values — the way designers author UI: a 50% alpha looks as it does in an image
// editor. Textures are sRGB, so sampling decodes them to linear; they are
// re-encoded here before the tint multiplies them. Output is premultiplied
// (blend: One, OneMinusSrcAlpha).

struct VertexIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0) var ui_texture: texture_2d<f32>;
@group(0) @binding(1) var ui_sampler: sampler;

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

// Linear → sRGB transfer (IEC 61966-2-1), per channel.
fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(ui_texture, ui_sampler, in.uv);
    let c = vec4<f32>(encode_srgb(texel.rgb), texel.a) * in.color;
    return vec4<f32>(c.rgb * c.a, c.a);
}
