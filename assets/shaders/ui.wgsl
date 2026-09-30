// ui.wgsl — the in-game UI pass (#418), with SDF text (#419).
//
// Draws laid-out UI graphics onto the finished frame, after post-FX. Vertices
// arrive in NDC with a straight-alpha tint in display (sRGB-encoded) space. The
// pass targets a NON-sRGB view of the frame, so blending happens on the encoded
// values — the way designers author UI: a 50% alpha looks as it does in an image
// editor. Textures are sRGB, so sampling decodes them to linear; they are
// re-encoded here before the tint multiplies them. Output is premultiplied
// (blend: One, OneMinusSrcAlpha).
//
// Text vertices (`sdf.x` = 1) sample a single-channel signed distance field
// instead: 0.5 is the glyph edge and the field reaches SPREAD atlas pixels either
// side. Fill, outline and glow are all cut from that one distance, antialiased
// over one screen pixel (fwidth), and composited glow < outline < fill.

// Must match `render::ui::text::sdf::SPREAD` (a unit test checks it).
const SPREAD: f32 = 12.0;

struct VertexIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) outline: vec4<f32>,
    @location(4) glow: vec4<f32>,
    @location(5) sdf: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) outline: vec4<f32>,
    @location(3) glow: vec4<f32>,
    @location(4) sdf: vec4<f32>,
};

@group(0) @binding(0) var ui_texture: texture_2d<f32>;
@group(0) @binding(1) var ui_sampler: sampler;

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip = vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.outline = in.outline;
    out.glow = in.glow;
    out.sdf = in.sdf;
    return out;
}

// Linear → sRGB transfer (IEC 61966-2-1), per channel.
fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// A straight-alpha colour at `coverage`, premultiplied.
fn premul(c: vec4<f32>, coverage: f32) -> vec4<f32> {
    let a = c.a * coverage;
    return vec4<f32>(c.rgb * a, a);
}

// Fill over outline over glow, from the signed distance `d` (atlas pixels,
// inside positive) with an antialiasing half-width `aa`.
fn sdf_text(in: VertexOut, d: f32, aa: f32) -> vec4<f32> {
    let body = d + in.sdf.y;
    let edge = body + in.sdf.z;
    let fill = premul(in.color, smoothstep(-aa, aa, body));
    let outline = premul(in.outline, smoothstep(-aa, aa, edge));
    var c = fill + outline * (1.0 - fill.a);
    let reach = max(in.sdf.w, 1e-4);
    let falloff = 1.0 - clamp(-edge / reach, 0.0, 1.0);
    let glow = premul(in.glow, falloff * falloff);
    return c + glow * (1.0 - c.a);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Sampled and differentiated unconditionally: both stay in uniform control flow.
    let texel = textureSample(ui_texture, ui_sampler, in.uv);
    let d = (texel.r - 0.5) * 2.0 * SPREAD;
    let aa = max(fwidth(d), 1e-4) * 0.5;
    let c = vec4<f32>(encode_srgb(texel.rgb), texel.a) * in.color;
    let image = vec4<f32>(c.rgb * c.a, c.a);
    return select(image, sdf_text(in, d, aa), in.sdf.x > 0.5);
}
