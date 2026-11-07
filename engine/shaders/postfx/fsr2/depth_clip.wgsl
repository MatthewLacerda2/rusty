// engine/shaders/postfx/fsr2/depth_clip.wgsl — FSR2's depth-clip pass (#1848): AMD
// FidelityFX Super Resolution 2.2.1's `ffx_fsr2_depth_clip.h`, ported by hand to
// WGSL. Copyright (c) 2022-2023 Advanced Micro Devices, Inc.; MIT, the notice in
// `LICENSE.txt` beside this file.
//
// Per render pixel: the prepared input colour (YCoCg, exposed) with the depth clip
// (how much of last frame was a different, nearer surface: a disocclusion) in
// `.w`, and the dilated reactive masks (the reactive mask spread over its 3×3 by
// colour similarity in `.x`, the motion divergence in `.y`). FSR2's
// transparency-and-composition mask is not an input (it reads 0), so it adds
// nothing to `.y`.
//
// The reactive input is #1845's mask (`reactive.wgsl`): a packed camera index plus
// that camera's reactivity, decoded per pixel as TAA's resolve does, so a world
// translucent under a higher camera's surface reads 0.

#import numeric
#import postfx::fsr2::common::{fsr, render_size, render_size_i, display_size, exposure, finite_rgb, saturate, prepare_rgb, rgb_to_ycocg, is_on_screen, clamp_load, clamp_uv, bilinear_sampling, BILINEAR_OFFSETS, view_space_depth, view_space_position, RECONSTRUCTED_DEPTH_BILINEAR_WEIGHT_THRESHOLD, FAR_DEPTH}

@group(1) @binding(0) var<storage, read> prev_depth: array<u32>;
@group(1) @binding(1) var t_dilated_motion: texture_2d<f32>;
@group(1) @binding(2) var t_dilated_depth: texture_2d<f32>;
@group(1) @binding(3) var t_reactive: texture_2d<f32>;
@group(1) @binding(4) var t_prev_motion: texture_2d<f32>;
@group(1) @binding(5) var t_velocity: texture_2d<f32>;
@group(1) @binding(6) var t_color: texture_2d<f32>;
@group(1) @binding(7) var s_linear: sampler;
@group(1) @binding(8) var prepared_out: texture_storage_2d<rgba16float, write>;
@group(1) @binding(9) var reactive_out: texture_storage_2d<rgba8unorm, write>;

const DEPTH_CLIP_BASE_SCALE: f32 = 4.0;

// `LoadReconstructedPrevDepth`; off the image reads 0, as an out-of-range fetch
// does on FSR2's.
fn reconstructed_prev_depth(p: vec2<i32>) -> f32 {
    let size = render_size_i();
    if (!is_on_screen(p, size)) {
        return 0.0;
    }
    return bitcast<f32>(prev_depth[u32(p.y) * u32(size.x) + u32(p.x)]);
}

// `LoadDilatedDepth`; off the image reads 0, as FSR2's fetch does.
fn dilated_depth(p: vec2<i32>) -> f32 {
    if (!is_on_screen(p, render_size_i())) {
        return 0.0;
    }
    return textureLoad(t_dilated_depth, p, 0).r;
}

fn dilated_motion(p: vec2<i32>) -> vec2<f32> {
    return textureLoad(t_dilated_motion, p, 0).xy;
}

fn input_motion(p: vec2<i32>) -> vec2<f32> {
    let m = textureLoad(t_velocity, p, 0).xy * fsr.jitter.zw;
    return select(vec2<f32>(0.0), m, numeric::finite2(m));
}

fn input_color(p: vec2<i32>) -> vec3<f32> {
    return finite_rgb(textureLoad(t_color, p, 0).rgb);
}

// The reactivity at `p` (#1845): what a translucent of `p`'s own camera, or of one
// above it, laid down there; 0 where none drew.
fn reactive(p: vec2<i32>) -> f32 {
    if (fsr.frame.z < 0.5) {
        return 0.0;
    }
    let v = textureLoad(t_reactive, p, 0).r;
    let drawn_by = floor(v);
    let camera = textureLoad(t_velocity, p, 0).w;
    return select(0.0, v - drawn_by, v > 0.0 && drawn_by >= camera);
}

fn compute_depth_clip(uv: vec2<f32>, current_depth: f32) -> f32 {
    let size = render_size_i();
    let current = view_space_depth(current_depth);
    let b = bilinear_sampling(uv, size);
    var depth = 0.0;
    var weight_sum = 0.0;
    for (var i = 0; i < 4; i++) {
        let at = b.base + BILINEAR_OFFSETS[i];
        let weight = b.weights[i];
        if (is_on_screen(at, size) && weight > RECONSTRUCTED_DEPTH_BILINEAR_WEIGHT_THRESHOLD) {
            let prev_sample = reconstructed_prev_depth(at);
            let prev = view_space_depth(prev_sample);
            let diff = current - prev;
            if (diff > 0.0) {
                let plane = max(prev_sample, current_depth);
                let centre = view_space_position(vec2<i32>(render_size() * 0.5), size, plane);
                let corner = view_space_position(vec2<i32>(0), size, plane);
                let half_width = length(render_size());
                let threshold = max(current, prev);
                let k_sep = 1.37e-05;
                let k_fov = length(corner) / length(centre);
                let required = k_sep * k_fov * half_width * threshold;
                let resolution = saturate(length(render_size()) / length(vec2<f32>(1920.0, 1080.0)));
                let power = mix(1.0, 3.0, resolution);
                depth += pow(saturate(required / diff), power) * weight;
                weight_sum += weight;
            }
        }
    }
    return select(0.0, saturate(1.0 - depth / weight_sum), weight_sum > 0.0);
}

fn compute_motion_divergence(p: vec2<i32>, size: vec2<i32>) -> f32 {
    var min_convergence = 1.0;
    let nucleus = input_motion(p);
    let nucleus_lr = length(nucleus * render_size());
    var max_velocity_uv = length(nucleus);
    if (nucleus_lr > 1e-02) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let motion = input_motion(clamp_load(p, vec2<i32>(x, y), size));
                var velocity_uv = length(motion);
                max_velocity_uv = max(velocity_uv, max_velocity_uv);
                velocity_uv = max(velocity_uv, max_velocity_uv);
                let convergence = dot(motion / velocity_uv, nucleus / velocity_uv);
                min_convergence = min(min_convergence, convergence);
            }
        }
    }
    return saturate(1.0 - min_convergence) * saturate(max_velocity_uv / 0.01);
}

fn compute_depth_divergence(p: vec2<i32>) -> f32 {
    let max_dist = FAR_DEPTH;
    var depth_max = 0.0;
    var depth_min = max_dist;
    var max_dist_found = false;
    for (var y = -1; y < 2; y++) {
        for (var x = -1; x < 2; x++) {
            let at = p + vec2<i32>(x, y);
            let on_screen = f32(is_on_screen(at, render_size_i()));
            let d = view_space_depth(dilated_depth(at)) * on_screen;
            max_dist_found = max_dist_found || d == max_dist;
            depth_min = min(depth_min, d);
            depth_max = max(depth_max, d);
        }
    }
    return (1.0 - depth_min / depth_max) * select(1.0, 0.0, max_dist_found);
}

fn compute_temporal_motion_divergence(p: vec2<i32>) -> f32 {
    let uv = (vec2<f32>(p) + 0.5) / render_size();
    let motion = dilated_motion(p);
    let size = render_size_i();
    let reprojected = clamp_uv(uv + motion, size, size);
    let prev = textureSampleLevel(t_prev_motion, s_linear, reprojected, 0.0).xy;
    let px_distance = length(motion * display_size());
    if (px_distance <= 1.0) {
        return 0.0;
    }
    let ratio = 1.0 - saturate(length(prev) / length(motion));
    return mix(0.0, ratio, saturate(pow(px_distance / 20.0, 3.0)));
}

// `PreProcessReactiveMasks`, the reactive mask alone (no transparency-and-
// composition one). A neighbour as black as a black centre is similar (FSR2's
// `0 / 0`).
fn pre_process_reactive_masks(p: vec2<i32>, motion_divergence: f32) {
    let reference = input_color(p);
    var factor = vec2<f32>(0.0, motion_divergence);
    var colors: array<vec3<f32>, 9>;
    var reactives: array<f32, 9>;
    var sum = 0.0;
    for (var y = -1; y < 2; y++) {
        for (var x = -1; x < 2; x++) {
            let at = clamp_load(p, vec2<i32>(x, y), render_size_i());
            let i = (y + 1) * 3 + x + 1;
            colors[i] = input_color(at);
            reactives[i] = reactive(at);
            sum += reactives[i];
        }
    }
    if (sum > 0.0) {
        for (var i = 0; i < 9; i++) {
            let c = colors[i];
            let max_len_sq = max(dot(reference, reference), dot(c, c));
            let similarity = select(1.0, dot(reference, c) / max_len_sq, max_len_sq > 0.0);
            let power_bias_max = 6.0;
            let similarity_power = 1.0 + (power_bias_max - similarity * power_bias_max);
            factor.x = max(factor.x, pow(reactives[i], similarity_power));
        }
    }
    textureStore(reactive_out, p, vec4<f32>(factor, 0.0, 0.0));
}

// `EvaluateSurface`.
fn evaluate_surface(p: vec2<i32>) -> f32 {
    let d0 = view_space_depth(reconstructed_prev_depth(p + vec2<i32>(0, -1)));
    let d1 = view_space_depth(reconstructed_prev_depth(p));
    let d2 = view_space_depth(reconstructed_prev_depth(p + vec2<i32>(0, 1)));
    return 1.0 - f32(((d0 - d1) > (d1 * 0.01)) && ((d1 - d2) > (d2 * 0.01)));
}

@compute @workgroup_size(8, 8)
fn cs_depth_clip(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    if (!is_on_screen(p, render_size_i())) {
        return;
    }
    let depth_uv = (vec2<f32>(p) + 0.5) / render_size();
    var motion = dilated_motion(p);
    // Discard tiny motion vectors.
    motion *= f32(length(motion * display_size()) > 0.01);
    let depth_clip = compute_depth_clip(depth_uv + motion, dilated_depth(p)) * evaluate_surface(p);
    let rgb = prepare_rgb(max(vec3<f32>(0.0), input_color(p)), exposure());
    textureStore(prepared_out, p, vec4<f32>(rgb_to_ycocg(rgb), depth_clip));
    // Render-size motion vectors: the divergence reads the pixel itself.
    let motion_divergence = compute_motion_divergence(p, render_size_i());
    let temporal = saturate(compute_temporal_motion_divergence(p) - compute_depth_divergence(p));
    pre_process_reactive_masks(p, max(temporal, motion_divergence));
}
