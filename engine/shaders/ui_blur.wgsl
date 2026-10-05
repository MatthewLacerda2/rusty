// ui_blur.wgsl — the UI backdrop's dual-filter blur (#426).
//
// Marius Bjørge's dual filter ("Bandwidth-Efficient Rendering", SIGGRAPH 2015):
// a chain of half-size downsamples (`fs_down`, 5 taps) then upsamples back
// (`fs_up`, 8 taps). Every level doubles the reach for a few taps per pixel, at a
// resolution that halves each step — so a wide frosted-glass blur costs a fraction
// of a gaussian. Fullscreen triangle; every read is by uv, sized from the source.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let x = f32((vid << 1u) & 2u);
    let y = f32(vid & 2u);
    out.uv = vec2<f32>(x, y);
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(src, src_sampler, uv);
}

// The centre four times, plus the four diagonal neighbours one source texel out.
@fragment
fn fs_down(in: VsOut) -> @location(0) vec4<f32> {
    let h = 1.0 / vec2<f32>(textureDimensions(src));
    var s = tap(in.uv) * 4.0;
    s += tap(in.uv - h) + tap(in.uv + h);
    s += tap(in.uv + vec2<f32>(h.x, -h.y)) + tap(in.uv + vec2<f32>(-h.x, h.y));
    return s / 8.0;
}

// A tent of eight taps around the centre: the four axis neighbours at two
// half-texels out, the four diagonals at one, weighted double.
@fragment
fn fs_up(in: VsOut) -> @location(0) vec4<f32> {
    let h = 0.5 / vec2<f32>(textureDimensions(src));
    var s = tap(in.uv + vec2<f32>(-2.0 * h.x, 0.0)) + tap(in.uv + vec2<f32>(2.0 * h.x, 0.0));
    s += tap(in.uv + vec2<f32>(0.0, -2.0 * h.y)) + tap(in.uv + vec2<f32>(0.0, 2.0 * h.y));
    s += (tap(in.uv + vec2<f32>(-h.x, h.y)) + tap(in.uv + vec2<f32>(h.x, h.y))) * 2.0;
    s += (tap(in.uv + vec2<f32>(h.x, -h.y)) + tap(in.uv + vec2<f32>(-h.x, -h.y))) * 2.0;
    return s / 12.0;
}
