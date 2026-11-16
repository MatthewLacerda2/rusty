// engine/shaders/postfx/ssr_hiz.wgsl — one level of SSR's min-depth pyramid (#2228,
// `render::postfx::effects::ssr::hiz`): each texel the nearest depth among the
// texels above it, so a march that is in front of a texel here is in front of
// every texel it covers (`lighting/ssr_march.wgsl`'s `clear`). Its first level
// reads the velocity target's depth (`.z`), each next one the level above (`.r`).
// Conservative at an odd edge: the last texel of a level whose source is odd
// takes the source's last row or column too, which a mip chain's floor halving
// would leave out of every texel.

#import fullscreen

// The level above (or the velocity target), as a one-level view.
@group(0) @binding(0) var t_src: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

// The nearest of the source texels texel `pos` covers, by `lane` of each.
fn nearest(pos: vec2<f32>, lane: vec4<f32>) -> f32 {
    let src = vec2<i32>(textureDimensions(t_src));
    let base = vec2<i32>(pos) * 2;
    // A texel whose block ends one short of an odd source's edge takes the edge too.
    let wide = select(vec2<i32>(2), vec2<i32>(3), base + 3 == src);
    var m = 3.0e38;
    for (var y = 0; y < wide.y; y = y + 1) {
        for (var x = 0; x < wide.x; x = x + 1) {
            let p = min(base + vec2<i32>(x, y), src - 1);
            m = min(m, dot(textureLoad(t_src, p, 0), lane));
        }
    }
    return m;
}

@fragment
fn fs_first(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(nearest(pos.xy, vec4<f32>(0.0, 0.0, 1.0, 0.0)), 0.0, 0.0, 0.0);
}

@fragment
fn fs_min(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(nearest(pos.xy, vec4<f32>(1.0, 0.0, 0.0, 0.0)), 0.0, 0.0, 0.0);
}
