// engine/shaders/mips.wgsl — one mip level from the level above it (#1322).
//
// A fullscreen triangle drawn into level N samples level N-1 bilinearly at each
// texel's centre, which for an even size is the middle of the 2×2 block above it:
// a box filter. What the texels hold decides the views the pass reads and writes:
// a colour map's sRGB views decode on sample and encode on write, so it averages in
// linear light; a data map's raw views average the stored values.

@group(0) @binding(0)
var t_above: texture_2d<f32>;
@group(0) @binding(1)
var s_linear: sampler;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_downsample(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let level = max(textureDimensions(t_above) / 2u, vec2<u32>(1u));
    return textureSampleLevel(t_above, s_linear, pos.xy / vec2<f32>(level), 0.0);
}
