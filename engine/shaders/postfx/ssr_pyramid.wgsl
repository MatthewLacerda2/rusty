// engine/shaders/postfx/ssr_pyramid.wgsl — one level of SSR's colour pyramid (#1736,
// `render::postfx::effects::ssr::pyramid`): each texel the average of the 2×2 texels
// above it (an odd level's last row or column has none below it and is left out).
// A non-finite texel (an emissive past `f16`'s range is +inf in the HDR image) is
// left out of the average too, so one hot pixel never turns a whole block of a
// coarse level, and every rough reflection read from it, into inf or NaN. SSR's march
// reads a rough surface's hit from the level whose texels are its cone's width.

#import fullscreen

// The level above, as a one-level view.
@group(0) @binding(0) var t_src: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

// Whether every lane of `v` is a number: its exponent bits are not all ones.
fn finite(v: vec4<f32>) -> bool {
    let exponent = bitcast<vec4<u32>>(v) & vec4<u32>(0x7f800000u);
    return all(exponent != vec4<u32>(0x7f800000u));
}

@fragment
fn fs_downsample(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let top = vec2<i32>(textureDimensions(t_src)) - 1;
    let base = vec2<i32>(pos.xy) * 2;
    var sum = vec4<f32>(0.0);
    var count = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        let p = min(base + vec2<i32>(i & 1, i >> 1u), top);
        let c = textureLoad(t_src, p, 0);
        if (finite(c)) {
            sum = sum + c;
            count = count + 1.0;
        }
    }
    return sum / max(count, 1.0);
}
