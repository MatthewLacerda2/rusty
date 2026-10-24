// engine/shaders/mips.wgsl — one mip level from the level above it (#1322).
//
// A fullscreen triangle drawn into level N box-filters level N-1: each texel
// averages the texels of the level above that its footprint covers. An even side
// halves exactly (2 texels, ½ each); an odd side 2n+1 maps to n, so each texel
// spans 2 + 1/n texels and takes 3 taps weighted by their overlap; a side of 1
// stays 1. What the texels hold decides the views the pass reads and writes: a
// colour map's sRGB views decode on load and encode on write, so it averages in
// linear light; a data map's raw views average the stored values.

#import fullscreen

@group(0) @binding(0)
var t_above: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(i);
}

// The weights of the (up to) three texels above, from `2 * i` on, that texel `i`
// of a `below`-wide side covers, `above` being the side above it.
fn tap_weights(i: u32, above: u32, below: u32) -> vec3<f32> {
    if (above == below) {
        return vec3<f32>(1.0, 0.0, 0.0);
    }
    if (above % 2u == 0u) {
        return vec3<f32>(0.5, 0.5, 0.0);
    }
    return vec3<f32>(f32(below - i), f32(below), f32(i + 1u)) / f32(above);
}

@fragment
fn fs_downsample(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let above = textureDimensions(t_above);
    let below = max(above / 2u, vec2<u32>(1u));
    let texel = vec2<u32>(pos.xy);
    let wx = tap_weights(texel.x, above.x, below.x);
    let wy = tap_weights(texel.y, above.y, below.y);
    // A side of 1 reads its one texel; any other starts at twice the texel.
    let origin = select(texel * 2u, texel, above == below);
    var sum = vec4<f32>(0.0);
    for (var y = 0u; y < 3u; y++) {
        for (var x = 0u; x < 3u; x++) {
            let w = wx[x] * wy[y];
            if (w > 0.0) {
                let at = min(origin + vec2<u32>(x, y), above - 1u);
                sum += w * textureLoad(t_above, vec2<i32>(at), 0);
            }
        }
    }
    return sum;
}
