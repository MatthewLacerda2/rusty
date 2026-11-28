// One level of SSR's two hit pyramids (`render::postfx::effects::ssr::pyramid`): the trace's hits
// and its spread. A hits texel, and the spread's traced share (x), averages the 2×2 above it (an
// odd level's last row or column is left out), so the resolve's `hits / traced` divides like by
// like. The spread's radius and count (yzw) are a 1-3-3-1 tent over the 4×4 around those: a box
// level holds a small cluster of hits in one texel or its neighbour as a sub-pixel jitter moves
// it, and the radius field jumped with it. A non-finite texel is left out of either filter, so
// one hot pixel never turns a coarse block, and every blur read from it, into inf or NaN.
//
// One fullscreen pass per level writes both pyramids' level, as two colour targets (render
// targets only, never storage, keeping framebuffer compression), from one-level views, so the
// level read is uniform across the pass (crates/render/src/CLAUDE.md, *Shaders*).

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

// The tent over `src`'s 4×4 around texel `id`'s 2×2, renormalised over finite in-level texels.
fn tent(src: texture_2d<f32>, id: vec2<u32>) -> vec4<f32> {
    let top = vec2<i32>(textureDimensions(src)) - 1;
    let base = vec2<i32>(id) * 2 - 1;
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    for (var i = 0; i < 16; i = i + 1) {
        let o = vec2<i32>(i & 3, i >> 2u);
        let p = base + o;
        if (any(p < vec2<i32>(0)) || any(p > top)) {
            continue;
        }
        let c = textureLoad(src, p, 0);
        let w = TENT[o.x] * TENT[o.y];
        if (numeric::finite4(c)) {
            sum = sum + c * w;
            weight = weight + w;
        }
    }
    return sum / max(weight, 1e-6);
}

const TENT = array<f32, 4>(1.0, 3.0, 3.0, 1.0);

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

@fragment
fn fs_downsample(@builtin(position) pos: vec4<f32>) -> Level {
    let id = vec2<u32>(pos.xy);
    let spread = vec4<f32>(average(spread_in, id).x, tent(spread_in, id).yzw);
    return Level(average(hits_in, id), spread);
}
