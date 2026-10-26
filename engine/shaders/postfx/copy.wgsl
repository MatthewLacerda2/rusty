// engine/shaders/postfx/copy.wgsl — a plain copy (#397): lands the LDR result of the
// chain's in-place passes (the authored effects) in the output when no finishing pass
// (FXAA) does. Those passes end in an internal target rather than the output so the
// result can also be kept as next frame's history. Binds the chain's IO layout.

#import fullscreen

@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    // Oversized triangle covering the screen; UVs in [0,1].
    var out: VsOut;
    out.uv = fullscreen::uv(vid);
    out.pos = fullscreen::triangle_down(vid);
    return out;
}

@fragment
fn fs_copy(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(t_color, s_color, in.uv).rgb, 1.0);
}
