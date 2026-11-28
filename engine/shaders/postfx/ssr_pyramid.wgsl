// One level of SSR's two hit pyramids (`render::postfx::effects::ssr::pyramid`): the trace's hits
// and its spread, each texel the average of the 2×2 texels above it in its own pyramid (an odd
// level's last row or column has none below it and is left out). A non-finite texel is left out of
// the average too, so one hot pixel never turns a whole block of a coarse level, and every blur the
// resolve reads from it, into inf or NaN.
//
// One fullscreen pass per level writes both pyramids' level, as two colour
// targets: the pyramids are render targets only, never storage, so a GPU keeps
// their framebuffer compression. Each pass binds one-level views, so the level
// read and written is uniform across it, never per pixel (crates/render/src/CLAUDE.md,
// *Shaders*).

#import fullscreen
#import numeric

// The level above, of each pyramid, as one-level views.
@group(0) @binding(0) var hits_in: texture_2d<f32>;
@group(0) @binding(1) var spread_in: texture_2d<f32>;

// The level written, of each pyramid.
struct Level {
    @location(0) hits: vec4<f32>,
    @location(1) spread: vec4<f32>,
};

// The mean of the finite texels among the 2×2 of `src` above texel `id`.
fn average(src: texture_2d<f32>, id: vec2<u32>) -> vec4<f32> {
    let top = vec2<i32>(textureDimensions(src)) - 1;
    let base = vec2<i32>(id) * 2;
    var sum = vec4<f32>(0.0);
    var count = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        let p = min(base + vec2<i32>(i & 1, i >> 1u), top);
        let c = textureLoad(src, p, 0);
        if (numeric::finite4(c)) {
            sum = sum + c;
            count = count + 1.0;
        }
    }
    return sum / max(count, 1.0);
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

@fragment
fn fs_downsample(@builtin(position) pos: vec4<f32>) -> Level {
    let id = vec2<u32>(pos.xy);
    return Level(average(hits_in, id), average(spread_in, id));
}
