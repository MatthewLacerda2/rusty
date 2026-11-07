// engine/shaders/postfx/fsr2/accumulate.wgsl — FSR2's accumulate pass (#1848): AMD
// FidelityFX Super Resolution 2.2.1's `ffx_fsr2_accumulate.h`, `ffx_fsr2_reproject.h`
// and `ffx_fsr2_postprocess_lock_status.h`, ported by hand to WGSL. Copyright (c)
// 2022-2023 Advanced Micro Devices, Inc.; MIT, the notice in `LICENSE.txt` beside
// this file.
//
// Per display pixel: last frame's upscaled colour reprojected (a Lanczos 2 read
// with deringing), this frame's upsampled sample (`upsample.wgsl`), the history
// rectified into the sample's variance box unless a lock or the luma's instability
// keeps it, and the two blended by the accumulated weight. It writes next frame's
// history (colour and temporal reactivity), lock status and luma history, and the
// upscaled output the rest of the chain reads, and clears the pixel's new lock.

#import postfx::fsr2::common::{fsr, render_size, render_size_i, display_size, display_size_i, downscale_factor, jitter, exposure, is_reset_frame, saturate, clamp_uv, prepare_rgb, unprepare_rgb, rgb_to_ycocg, ycocg_to_rgb, tonemap, inverse_tonemap, min_divided_by_max, lanczos2, EPSILON, MAX_ACCUMULATION_LANCZOS_WEIGHT, AVERAGE_LANCZOS_WEIGHT_PER_FRAME, LUMA_MIP_DIV}
#import postfx::fsr2::upsample::{Box, upsampled}

@group(1) @binding(0) var t_prepared: texture_2d<f32>;
@group(1) @binding(1) var t_reactive_masks: texture_2d<f32>;
@group(1) @binding(2) var t_dilated_motion: texture_2d<f32>;
@group(1) @binding(3) var t_history: texture_2d<f32>;
@group(1) @binding(4) var t_lock_status: texture_2d<f32>;
@group(1) @binding(5) var t_luma_history: texture_2d<f32>;
@group(1) @binding(6) var t_luma_mip: texture_2d<f32>;
@group(1) @binding(7) var s_linear: sampler;
@group(1) @binding(8) var<storage, read_write> new_locks: array<u32>;
@group(1) @binding(9) var history_out: texture_storage_2d<rgba16float, write>;
@group(1) @binding(10) var lock_out: texture_storage_2d<rgba16float, write>;
@group(1) @binding(11) var luma_history_out: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(12) var upscaled_out: texture_storage_2d<rgba16float, write>;

// `AccumulationPassCommonParams`.
struct Params {
    hr: vec2<i32>,
    hr_uv: vec2<f32>,
    lr_uv: vec2<f32>,
    motion: vec2<f32>,
    reprojected_uv: vec2<f32>,
    hr_velocity: f32,
    depth_clip: f32,
    dilated_reactive: f32,
    accumulation_mask: f32,
    reset: bool,
    existing: bool,
    new_sample: bool,
};

fn is_uv_inside(uv: vec2<f32>) -> bool {
    return all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
}

fn init_params(hr: vec2<i32>) -> Params {
    var p: Params;
    p.hr = hr;
    p.hr_uv = (vec2<f32>(hr) + 0.5) / display_size();
    let size = render_size_i();
    p.lr_uv = clamp_uv(p.hr_uv + jitter() / render_size(), size, size);
    // Render-size motion vectors: the dilated one under the pixel.
    p.motion = textureLoad(t_dilated_motion, vec2<i32>(p.hr_uv * render_size()), 0).xy;
    p.hr_velocity = length(p.motion * display_size());
    p.reprojected_uv = p.hr_uv + p.motion;
    p.existing = is_uv_inside(p.reprojected_uv);
    p.depth_clip = saturate(textureSampleLevel(t_prepared, s_linear, p.lr_uv, 0.0).w);
    let masks = textureSampleLevel(t_reactive_masks, s_linear, p.lr_uv, 0.0).xy;
    p.dilated_reactive = masks.x;
    p.accumulation_mask = masks.y;
    p.reset = is_reset_frame();
    p.new_sample = !p.existing || p.reset;
    return p;
}

fn history_texel(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(t_history, clamp(p, vec2<i32>(0), display_size_i() - 1), 0);
}

fn lanczos2_row(c0: vec4<f32>, c1: vec4<f32>, c2: vec4<f32>, c3: vec4<f32>, t: f32) -> vec4<f32> {
    let w = vec4<f32>(lanczos2(-1.0 - t), lanczos2(-t), lanczos2(1.0 - t), lanczos2(2.0 - t));
    return (w.x * c0 + w.y * c1 + w.z * c2 + w.w * c3) / (w.x + w.y + w.z + w.w);
}

// `HistorySample`: a reference Lanczos 2 over the 4×4 around `uv`, clamped into
// the inner 2×2's range (deringing).
fn history_sample(uv: vec2<f32>) -> vec4<f32> {
    let size = display_size();
    let px = clamp(uv * size - 0.5, vec2<f32>(0.0), size);
    let base = vec2<i32>(floor(px));
    let f = fract(px);
    var rows: array<vec4<f32>, 4>;
    for (var y = 0; y < 4; y++) {
        let at = base + vec2<i32>(-1, y - 1);
        rows[y] = lanczos2_row(
            history_texel(at),
            history_texel(at + vec2<i32>(1, 0)),
            history_texel(at + vec2<i32>(2, 0)),
            history_texel(at + vec2<i32>(3, 0)),
            f.x,
        );
    }
    let c = lanczos2_row(rows[0], rows[1], rows[2], rows[3], f.y);
    let a = history_texel(base);
    let b = history_texel(base + vec2<i32>(1, 0));
    let d = history_texel(base + vec2<i32>(0, 1));
    let e = history_texel(base + vec2<i32>(1, 1));
    return clamp(c, min(min(a, b), min(d, e)), max(max(a, b), max(d, e)));
}

// What the reprojection reads of last frame: its colour (YCoCg, exposed), its
// temporal reactivity, whether it was moving, and its lock status.
struct Reprojected {
    color: vec3<f32>,
    temporal_reactive: f32,
    in_motion: bool,
    lock: vec2<f32>,
    new_lock: bool,
    was_locked: bool,
};

fn reproject(p: Params) -> Reprojected {
    var r = Reprojected(vec3<f32>(0.0), 0.0, false, vec2<f32>(0.0), false, false);
    if (p.existing && !p.reset) {
        let h = history_sample(p.reprojected_uv);
        r.color = rgb_to_ycocg(prepare_rgb(h.rgb, exposure()));
        r.temporal_reactive = saturate(abs(h.w));
        r.in_motion = h.w < 0.0;
        let index = u32(p.hr.y) * u32(display_size_i().x) + u32(p.hr.x);
        r.new_lock = new_locks[index] != 0u;
        r.lock = textureSampleLevel(t_lock_status, s_linear, p.reprojected_uv, 0.0).xy;
        r.was_locked = r.lock.x != 0.0;
    }
    return r;
}

// `GetShadingChangeLuma`.
fn shading_change_luma(uv_in: vec2<f32>) -> f32 {
    let mip_size = max(vec2<i32>(render_size() / LUMA_MIP_DIV), vec2<i32>(1));
    let uv = clamp_uv(uv_in, mip_size, vec2<i32>(textureDimensions(t_luma_mip)));
    let luma = exposure() * exp(textureSampleLevel(t_luma_mip, s_linear, uv, 0.0).r);
    return pow(luma, 1.0 / 6.0);
}

// What `UpdateLockStatus` leaves: the lock status, this frame's lock contribution
// and the shading change's luma difference.
struct LockUpdate {
    lock: vec2<f32>,
    contribution: f32,
    luminance_diff: f32,
};

// `UpdateLockStatus`; raises `reactive` by the shading change.
fn update_lock(p: Params, reactive: ptr<function, f32>, r: Reprojected) -> LockUpdate {
    var lock = r.lock;
    let luma = shading_change_luma(p.hr_uv);
    lock.y = select(lock.y, luma, lock.y == 0.0);
    let previous = lock.y;
    let diff = 1.0 - min_divided_by_max(previous, luma);
    if (r.new_lock) {
        lock.y = luma;
        lock.x = select(1.0, 2.0, lock.x != 0.0);
    } else if (lock.x <= 1.0) {
        lock.y = mix(lock.y, luma, 0.5);
    } else if (diff > 0.1) {
        lock.x = 0.0;
    }
    *reactive = max(*reactive, saturate((diff - 0.1) * 10.0));
    lock.x *= 1.0 - *reactive;
    lock.x *= saturate(1.0 - p.accumulation_mask);
    lock.x *= f32(p.depth_clip < 0.1);
    let lifetime = saturate(lock.x - 1.0);
    let shading = saturate(min_divided_by_max(lock.y, luma));
    return LockUpdate(lock, saturate(saturate(lifetime * 4.0) * shading), diff);
}

// `ComputeLumaInstabilityFactor`; writes next frame's luma history.
fn luma_instability(p: Params, rect: Box, reactive: f32, luminance_diff: f32) -> f32 {
    let byte_step = 1.0 / 255.0;
    var luma = rect.center.x;
    luma = luma / (1.0 + max(0.0, luma));
    luma = floor(luma * 255.0 + 0.5) / 255.0;
    let sample_history = max(max(p.depth_clip, p.accumulation_mask), luminance_diff) < 0.1 && !p.new_sample;
    var history = vec4<f32>(0.0);
    if (sample_history) {
        history = textureSampleLevel(t_luma_history, s_linear, p.reprojected_uv, 0.0);
    }
    var instability = 0.0;
    let diff0 = luma - history.x;
    var least = abs(diff0);
    if (least >= byte_step) {
        for (var i = 1; i <= 3; i++) {
            let diff = luma - history[i];
            if (sign(diff0) == sign(diff)) {
                least = min(least, abs(diff));
            }
        }
        let box_size_factor = pow(saturate(rect.vec.x / 0.1), 6.0);
        instability = f32(least != abs(diff0)) * box_size_factor;
        instability = f32(instability > byte_step);
        instability *= 1.0 - max(p.accumulation_mask, pow(reactive, 1.0 / 6.0));
    }
    history = vec4<f32>(luma, history.x, history.y, history.z);
    textureStore(luma_history_out, p.hr, history);
    return instability * f32(history.w != 0.0);
}

fn base_accumulation(p: Params, reactive: f32, in_motion: bool, weight: f32) -> vec3<f32> {
    var base = MAX_ACCUMULATION_LANCZOS_WEIGHT * f32(p.existing) * (1.0 - reactive) * (1.0 - p.depth_clip);
    base = min(base, mix(base, weight * 10.0, max(f32(in_motion), saturate(p.hr_velocity * 10.0))));
    base = min(base, mix(base, weight, saturate(p.hr_velocity / 20.0)));
    return vec3<f32>(base);
}

// `RectifyHistory`; may lower `accumulation`.
fn rectify(p: Params, rect: Box, history: vec3<f32>, accumulation: ptr<function, vec3<f32>>, lock: f32, instability: f32) -> vec3<f32> {
    let downscale = downscale_factor();
    let scale_influence = min(20.0, pow(1.0 / abs(downscale.x * downscale.y), 3.0));
    let velocity_factor = saturate(p.hr_velocity / 20.0);
    let box_scale_t = max(p.depth_clip, max(p.accumulation_mask, velocity_factor));
    let box_scale = mix(scale_influence, 1.0, box_scale_t);
    let scaled = rect.vec * box_scale;
    let box_min = max(rect.aabb_min, rect.center - scaled);
    let box_max = min(rect.aabb_max, rect.center + scaled);
    if (any(box_min > history) || any(history > box_max)) {
        let clamped = clamp(history, box_min, box_max);
        let contribution = max(instability, lock) * (1.0 - sqrt(p.dilated_reactive));
        let t = saturate(contribution);
        *accumulation = mix(min(*accumulation, vec3<f32>(0.1)), *accumulation, t);
        return mix(clamped, history, t);
    }
    return history;
}

// `Accumulate` with HDR input: the blend in tonemapped RGB, back to linear RGB.
fn blend(history: vec3<f32>, accumulation_in: vec3<f32>, current_sample: vec4<f32>) -> vec3<f32> {
    let accumulation = max(vec3<f32>(EPSILON), accumulation_in + current_sample.www);
    let current = rgb_to_ycocg(tonemap(ycocg_to_rgb(current_sample.xyz)));
    let past = rgb_to_ycocg(tonemap(ycocg_to_rgb(history)));
    let alpha = current_sample.www / accumulation;
    return inverse_tonemap(ycocg_to_rgb(mix(past, current, alpha)));
}

fn temporal_reactive(p: Params, factor: f32) -> f32 {
    var f = min(0.99, factor);
    f = max(f, mix(f, 0.4, saturate(p.hr_velocity)));
    f = max(f * f, max(p.depth_clip * 0.1, p.dilated_reactive));
    f = select(f, 1.0, p.new_sample);
    if (saturate(p.hr_velocity * 10.0) >= 1.0) {
        f = max(EPSILON, f) * -1.0;
    }
    return f;
}

// `FinalizeLockStatus`.
fn finalize_lock(p: Params, lock_in: vec2<f32>, weight: f32) -> vec2<f32> {
    var lock = lock_in;
    if (!is_uv_inside(p.hr_uv - p.motion)) {
        lock.x = 0.0;
    } else {
        let decrease_max = fsr.frame.y * AVERAGE_LANCZOS_WEIGHT_PER_FRAME;
        lock.x = max(0.0, lock.x - weight / decrease_max);
    }
    return lock;
}

@compute @workgroup_size(8, 8)
fn cs_accumulate(@builtin(global_invocation_id) id: vec3<u32>) {
    let hr = vec2<i32>(id.xy);
    let display = display_size_i();
    if (any(hr >= display)) {
        return;
    }
    let p = init_params(hr);
    let r = reproject(p);
    var reactive = max(p.dilated_reactive, r.temporal_reactive);
    let lock = update_lock(p, &reactive, r);
    let up = upsampled(t_prepared, hr, reactive, p.new_sample, p.depth_clip, p.hr_velocity);
    let instability = luma_instability(p, up.rect, reactive, lock.luminance_diff);
    var accumulation = base_accumulation(p, reactive, r.in_motion, up.color_and_weight.w);
    var color: vec3<f32>;
    if (p.new_sample) {
        color = ycocg_to_rgb(up.color_and_weight.xyz);
    } else {
        let rectified = rectify(p, up.rect, r.color, &accumulation, lock.contribution, instability);
        color = blend(rectified, accumulation, up.color_and_weight);
    }
    color = unprepare_rgb(color, exposure());
    let final_lock = finalize_lock(p, lock.lock, up.color_and_weight.w);
    textureStore(lock_out, hr, vec4<f32>(final_lock, 0.0, 0.0));
    textureStore(history_out, hr, vec4<f32>(color, temporal_reactive(p, reactive)));
    textureStore(upscaled_out, hr, vec4<f32>(color, 1.0));
    new_locks[u32(hr.y) * u32(display.x) + u32(hr.x)] = 0u;
}
