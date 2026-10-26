// engine/shaders/postfx/bloom.wgsl — bloom's two passes, before the composite
// (`render::postfx::effects::bloom`): the bright-pass threshold (prefilter) and a 2D
// gaussian blur, both at the bloom divisor's reduced resolution. The composite adds the
// blurred result (`postfx/effects/bloom.wgsl`).

#import fullscreen
#import postfx::grade::luminance

// Mirrors the Rust `BloomParams`.
struct BloomParams {
    // x: the threshold, yz: the blurred target's texel size, w: unused
    knobs: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: BloomParams;
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

// ---- Bright-pass (bloom prefilter) ----
@fragment
fn fs_bright(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(t_color, s_color, in.uv).rgb;
    let threshold = params.knobs.x;
    let l = luminance(c);
    let factor = max(l - threshold, 0.0) / max(l, 0.0001);
    return vec4<f32>(c * factor, 1.0);
}

// 1D gaussian weight by absolute offset (0..2). A function avoids dynamic array
// indexing, which naga forbids on function-local arrays.
fn gauss_w(i: i32) -> f32 {
    if (i == 0) { return 0.227027; }
    if (i == 1 || i == -1) { return 0.3162162; }
    return 0.0702703;
}

// ---- 2D gaussian blur (single pass; cheap at the reduced bloom resolution) ----
@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4<f32> {
    let tx = params.knobs.y;
    let ty = params.knobs.z;
    var result = vec3<f32>(0.0);
    for (var x = -2; x <= 2; x = x + 1) {
        for (var y = -2; y <= 2; y = y + 1) {
            let off = vec2<f32>(f32(x) * tx, f32(y) * ty);
            let weight = gauss_w(x) * gauss_w(y);
            // textureSampleLevel (explicit LOD 0), not textureSample: a gradient
            // (implicit-LOD) instruction inside a loop fails to compile on D3D12
            // (FXC "gradient instruction used in a loop"). These full-screen
            // targets have no mips, so LOD 0 is identical on every backend.
            result += textureSampleLevel(t_color, s_color, in.uv + off, 0.0).rgb * weight;
        }
    }
    return vec4<f32>(result, 1.0);
}
