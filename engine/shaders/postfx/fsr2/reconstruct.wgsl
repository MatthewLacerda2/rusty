// engine/shaders/postfx/fsr2/reconstruct.wgsl — FSR2's reconstruct-and-dilate pass
// (#1848): AMD FidelityFX Super Resolution 2.2.1's
// `ffx_fsr2_reconstruct_dilated_velocity_and_previous_depth.h`, ported by hand to
// WGSL. Copyright (c) 2022-2023 Advanced Micro Devices, Inc.; MIT, the notice in
// `LICENSE.txt` beside this file.
//
// Per render pixel: the nearest depth of its 3×3 and that texel's motion (the
// dilated depth and motion), the nearest depth pushed to where it was last frame
// (FSR2's atomic min into an `r32ui` image; here a storage buffer of atomics, which
// core WGSL has), and the luma the lock pass detects thin features in.
//
// Inputs: the velocity target (`.xy` NDC motion, unjittered; `.z` the view depth,
// FSR2's depth here, `common.wgsl`), the scene's HDR colour.

#import numeric
#import postfx::fsr2::common::{fsr, render_size, render_size_i, display_size, exposure, finite_rgb, tonemap, rgb_to_perceived_luma, is_on_screen, bilinear_sampling, BILINEAR_OFFSETS, RECONSTRUCTED_DEPTH_BILINEAR_WEIGHT_THRESHOLD, FAR_DEPTH}

@group(1) @binding(0) var t_velocity: texture_2d<f32>;
@group(1) @binding(1) var t_color: texture_2d<f32>;
@group(1) @binding(2) var<storage, read_write> prev_depth: array<atomic<u32>>;
@group(1) @binding(3) var dilated_depth: texture_storage_2d<r32float, write>;
@group(1) @binding(4) var dilated_motion: texture_storage_2d<rgba16float, write>;
@group(1) @binding(5) var lock_luma: texture_storage_2d<r32float, write>;

// `LoadInputMotionVector`: FSR2's uv motion (last frame's uv minus this one's) at
// render pixel `p`; a non-finite one reads none.
fn input_motion(p: vec2<i32>) -> vec2<f32> {
    let m = textureLoad(t_velocity, p, 0).xy * fsr.jitter.zw;
    return select(vec2<f32>(0.0), m, numeric::finite2(m));
}

// `LoadInputDepth`: the view depth at `p`, clamped to the sky's.
fn input_depth(p: vec2<i32>) -> f32 {
    let z = textureLoad(t_velocity, p, 0).z;
    return select(FAR_DEPTH, clamp(z, 0.0, FAR_DEPTH), numeric::finite2(vec2<f32>(z, 0.0)));
}

fn reconstruct_prev_depth(p: vec2<i32>, depth: f32, motion_in: vec2<f32>, size: vec2<i32>) {
    let motion = motion_in * f32(length(motion_in * display_size()) > 0.1);
    let uv = (vec2<f32>(p) + 0.5) / vec2<f32>(size);
    let b = bilinear_sampling(uv + motion, render_size_i());
    for (var i = 0; i < 4; i++) {
        if (b.weights[i] > RECONSTRUCTED_DEPTH_BILINEAR_WEIGHT_THRESHOLD) {
            let at = b.base + BILINEAR_OFFSETS[i];
            if (is_on_screen(at, size)) {
                let index = u32(at.y) * u32(size.x) + u32(at.x);
                atomicMin(&prev_depth[index], bitcast<u32>(depth));
            }
        }
    }
}

const NEAREST_OFFSETS: array<vec2<i32>, 9> = array<vec2<i32>, 9>(
    vec2<i32>(0, 0),
    vec2<i32>(1, 0),
    vec2<i32>(0, 1),
    vec2<i32>(0, -1),
    vec2<i32>(-1, 0),
    vec2<i32>(-1, 1),
    vec2<i32>(1, 1),
    vec2<i32>(-1, -1),
    vec2<i32>(1, -1),
);

struct Nearest {
    depth: f32,
    at: vec2<i32>,
};

// `FindNearestDepth`, its depth never inverted (`common.wgsl`).
fn find_nearest_depth(p: vec2<i32>, size: vec2<i32>) -> Nearest {
    var nearest = Nearest(input_depth(p), p);
    for (var i = 1; i < 9; i++) {
        let at = p + NEAREST_OFFSETS[i];
        if (is_on_screen(at, size)) {
            let d = input_depth(at);
            if (d < nearest.depth) {
                nearest = Nearest(d, at);
            }
        }
    }
    return nearest;
}

// `ComputeLockInputLuma`, with HDR input.
fn lock_input_luma(p: vec2<i32>) -> f32 {
    let rgb = max(vec3<f32>(0.0), finite_rgb(textureLoad(t_color, p, 0).rgb)) * exposure();
    return pow(rgb_to_perceived_luma(tonemap(rgb)), 1.0 / 6.0);
}

@compute @workgroup_size(8, 8)
fn cs_reconstruct(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    let size = render_size_i();
    if (!is_on_screen(p, size)) {
        return;
    }
    let nearest = find_nearest_depth(p, size);
    // Render-size motion vectors (`FFX_FSR2_OPTION_LOW_RESOLUTION_MOTION_VECTORS`).
    let motion = input_motion(nearest.at);
    textureStore(dilated_depth, p, vec4<f32>(nearest.depth, 0.0, 0.0, 0.0));
    textureStore(dilated_motion, p, vec4<f32>(motion, 0.0, 0.0));
    reconstruct_prev_depth(p, nearest.depth, motion, size);
    textureStore(lock_luma, p, vec4<f32>(lock_input_luma(p), 0.0, 0.0, 0.0));
}
