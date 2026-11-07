// engine/shaders/postfx/fsr2/luma.wgsl — FSR2's luminance pyramid (#1848), the one
// level a later pass reads: AMD FidelityFX Super Resolution 2.2.1's
// `ffx_fsr2_compute_luminance_pyramid.h`, ported by hand to WGSL. Copyright (c)
// 2022-2023 Advanced Micro Devices, Inc.; MIT, the notice in `LICENSE.txt` beside
// this file.
//
// FSR2 reduces the frame's log luminance with SPD (one dispatch, a global atomic
// counter and `globallycoherent` mips) for its auto exposure and keeps two levels:
// the 1×1 (the exposure, off here: the engine's runs) and the shading-change level
// the accumulate pass's locks read, each texel the mean log luminance of 32×32
// render pixels (out-of-image pixels count zero). This pass writes that level
// directly: one workgroup per texel, each invocation a 2×2 quad of samples, summed
// in workgroup memory. The same mean, without what core WGSL lacks.

#import postfx::fsr2::common::{fsr, jitter, render_size, render_size_i, clamp_uv, rgb_to_luma, finite_rgb, EPSILON, LUMA_MIP_DIV}

@group(1) @binding(0) var t_color: texture_2d<f32>;
@group(1) @binding(1) var s_linear: sampler;
@group(1) @binding(2) var luma_out: texture_storage_2d<rgba16float, write>;

var<workgroup> partial: array<f32, 256>;

// `SpdLoadSourceImage`: the log luminance of render pixel `tex`, read at its
// unjittered place; zero outside the image.
fn source_log_luma(tex: vec2<f32>) -> f32 {
    let size = render_size_i();
    let uv = clamp_uv((tex + 0.5 + jitter()) / render_size(), size, size);
    let rgb = finite_rgb(textureSampleLevel(t_color, s_linear, uv, 0.0).rgb);
    let log_luma = log(max(EPSILON, rgb_to_luma(rgb)));
    return select(0.0, log_luma, all(tex < render_size()));
}

@compute @workgroup_size(16, 16)
fn cs_luma(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(local_invocation_index) index: u32,
) {
    let base = vec2<f32>(group.xy * 32u + local.xy * 2u);
    var sum = 0.0;
    for (var y = 0; y < 2; y++) {
        for (var x = 0; x < 2; x++) {
            sum += source_log_luma(base + vec2<f32>(f32(x), f32(y)));
        }
    }
    partial[index] = sum;
    workgroupBarrier();
    for (var stride = 128u; stride > 0u; stride = stride >> 1u) {
        if (index < stride) {
            partial[index] += partial[index + stride];
        }
        workgroupBarrier();
    }
    if (index == 0u) {
        let mean = partial[0] / (LUMA_MIP_DIV * LUMA_MIP_DIV);
        textureStore(luma_out, vec2<i32>(group.xy), vec4<f32>(mean, 0.0, 0.0, 0.0));
    }
}
