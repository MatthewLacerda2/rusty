// engine/shaders/postfx/fsr2/upsample.wgsl — FSR2's upsample (#1848): AMD FidelityFX
// Super Resolution 2.2.1's `ffx_fsr2_upsample.h` and the rectification box of
// `ffx_fsr2_common.h`, ported by hand to WGSL. Copyright (c) 2022-2023 Advanced
// Micro Devices, Inc.; MIT, the notice in `LICENSE.txt` beside this file.
//
// One display pixel's sample of this frame: the 3×3 render pixels around it
// (FSR2's 4×4 window, flipped so the 3×3 nearest the pixel are read), weighted by
// an approximate Lanczos 2 whose kernel widens with the upscale and narrows where
// the pixel is reactive or new, and the variance box the history is rectified into.

#define_import_path postfx::fsr2::upsample

#import postfx::fsr2::common::{jitter, downscale_factor, render_size_i, is_on_screen, lanczos2_approx_sq, saturate, FLT_MAX, EPSILON, UPSAMPLE_LANCZOS_WEIGHT_SCALE}

// `RectificationBox`: the neighbourhood's weighted mean (`center`), deviation
// (`vec`, after `finish_box`) and min/max.
struct Box {
    center: vec3<f32>,
    vec: vec3<f32>,
    aabb_min: vec3<f32>,
    aabb_max: vec3<f32>,
    weight: f32,
};

fn add_sample(b: ptr<function, Box>, initial: bool, c: vec3<f32>, w: f32) {
    if (initial) {
        (*b).aabb_min = c;
        (*b).aabb_max = c;
        (*b).center = c * w;
        (*b).vec = c * c * w;
        (*b).weight = w;
    } else {
        (*b).aabb_min = min((*b).aabb_min, c);
        (*b).aabb_max = max((*b).aabb_max, c);
        (*b).center += c * w;
        (*b).vec += c * c * w;
        (*b).weight += w;
    }
}

// `RectificationBoxComputeVarianceBoxData`.
fn finish_box(b: ptr<function, Box>) {
    (*b).weight = select(1.0, (*b).weight, abs((*b).weight) > EPSILON);
    (*b).center /= (*b).weight;
    (*b).vec /= (*b).weight;
    (*b).vec = sqrt(abs((*b).vec - (*b).center * (*b).center));
}

// `ComputeMaxKernelWeight`.
fn max_kernel_weight() -> f32 {
    let bias = 1.0;
    let weight = 1.0 + (1.0 / downscale_factor() - 1.0).x * bias;
    return min(1.99, weight);
}

// What `ComputeUpsampledColorAndWeight` returns: the sample (YCoCg) and its
// weight, and the box.
struct Upsampled {
    color_and_weight: vec4<f32>,
    rect: Box,
};

// `ComputeUpsampledColorAndWeight` for display pixel `hr`, from the prepared input
// colour `prepared`; `reactive` and `new_sample` narrow the kernel, `depth_clip`
// and `hr_velocity` (display pixels a frame) shape it and the box.
fn upsampled(
    prepared: texture_2d<f32>,
    hr: vec2<i32>,
    reactive: f32,
    new_sample: bool,
    depth_clip: f32,
    hr_velocity: f32,
) -> Upsampled {
    let dst = vec2<f32>(hr) + 0.5;
    let src = dst * downscale_factor();
    let src_px = vec2<i32>(floor(src));
    let unjittered = (vec2<f32>(src_px) + 0.5) - jitter();
    let flip_col = unjittered.x > src.x;
    let flip_row = unjittered.y > src.y;
    let offset_tl = vec2<i32>(select(-1, -2, flip_col), select(-1, -2, flip_row));
    let size = render_size_i();
    var samples: array<vec3<f32>, 16>;
    for (var row = 0; row < 3; row++) {
        for (var col = 0; col < 3; col++) {
            let cr = vec2<i32>(select(col, 3 - col, flip_col), select(row, 3 - row, flip_row));
            // FSR2's `ClampLoad` with a zero offset clamps nothing: a tap off the
            // image reads 0, as its out-of-range fetch does.
            let at = src_px + offset_tl + cr;
            let inside = is_on_screen(at, size);
            let texel = textureLoad(prepared, clamp(at, vec2<i32>(0), size - 1), 0).xyz;
            samples[col + (row << 2u)] = select(vec3<f32>(0.0), texel, inside);
        }
    }
    var color_and_weight = vec4<f32>(0.0);
    let base_offset = unjittered - src;
    let kernel_reactive = max(reactive, f32(new_sample));
    let bias_max = max_kernel_weight() * (1.0 - kernel_reactive);
    let bias_min = max(1.0, (1.0 + bias_max) * 0.3);
    let bias_factor = max(0.0, max(0.25 * depth_clip, kernel_reactive));
    let kernel_bias = mix(bias_max, bias_min, bias_factor);
    let curve_bias = mix(-2.0, -3.0, saturate(hr_velocity / 50.0));
    var rect: Box;
    for (var row = 0; row < 3; row++) {
        for (var col = 0; col < 3; col++) {
            let i = col + (row << 2u);
            let cr = vec2<i32>(select(col, 3 - col, flip_col), select(row, 3 - row, flip_row));
            let offset = base_offset + vec2<f32>(offset_tl + cr);
            let at = src_px + offset_tl + cr;
            let on_screen = f32(is_on_screen(at, size));
            let biased = offset * kernel_bias;
            let weight = on_screen * lanczos2_approx_sq(dot(biased, biased));
            color_and_weight += vec4<f32>(samples[i] * weight, weight);
            let box_weight = exp(curve_bias * dot(offset, offset));
            add_sample(&rect, row == 0 && col == 0, samples[i], box_weight);
        }
    }
    finish_box(&rect);
    color_and_weight.w *= f32(color_and_weight.w > EPSILON);
    if (color_and_weight.w > EPSILON) {
        color_and_weight = vec4<f32>(
            // Deringing: into the neighbourhood's min/max.
            clamp(color_and_weight.xyz / color_and_weight.w, rect.aabb_min, rect.aabb_max),
            color_and_weight.w * UPSAMPLE_LANCZOS_WEIGHT_SCALE,
        );
    }
    return Upsampled(color_and_weight, rect);
}
