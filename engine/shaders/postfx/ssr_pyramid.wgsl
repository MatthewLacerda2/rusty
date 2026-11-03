// engine/shaders/postfx/ssr_pyramid.wgsl — one level of SSR's colour pyramid (#1736,
// `render::postfx::effects::ssr::pyramid`): each texel the 2×2 box average of the
// level above, by one bilinear tap at their shared corner. SSR's march reads a rough
// surface's hit from the level whose texels are its cone's width.

#import fullscreen

// The level above, as a one-level view, and a linear clamp sampler.
@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

@fragment
fn fs_downsample(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let size = max(textureDimensions(t_src) / 2u, vec2<u32>(1u));
    let uv = pos.xy / vec2<f32>(size);
    return textureSampleLevel(t_src, s_src, uv, 0.0);
}
