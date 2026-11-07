// engine/shaders/postfx/fsr2/lock.wgsl — FSR2's lock pass (#1848): AMD FidelityFX
// Super Resolution 2.2.1's `ffx_fsr2_lock.h`, ported by hand to WGSL. Copyright (c)
// 2022-2023 Advanced Micro Devices, Inc.; MIT, the notice in `LICENSE.txt` beside
// this file.
//
// Per render pixel: a thin feature (a luma ridge no 2×2 of similar neighbours
// covers) locks the display pixel its sample lands in, so the accumulate pass
// keeps it through the history's clamping; then the reconstructed previous depth
// is cleared to the far plane for next frame's atomic min. FSR2's `r8` new-locks
// image and `r32ui` depth image are storage buffers here (core WGSL has no
// read-write storage texture of either format).

#import postfx::fsr2::common::{render_size_i, display_size_i, clamp_load, is_on_screen, compute_hr_pos_from_lr_pos, FLT_MAX, FAR_DEPTH}

@group(1) @binding(0) var t_lock_luma: texture_2d<f32>;
@group(1) @binding(1) var<storage, read_write> new_locks: array<u32>;
@group(1) @binding(2) var<storage, read_write> prev_depth: array<u32>;

fn lock_luma(p: vec2<i32>) -> f32 {
    return textureLoad(t_lock_luma, p, 0).r;
}

// `ComputeThinFeatureConfidence`: the bits of the 3×3 (row by row, 4 the centre)
// that hold a luma similar to the centre's.
fn thin_feature(p: vec2<i32>) -> bool {
    let nucleus = lock_luma(p);
    let similar_threshold = 1.05;
    var dissimilar_min = FLT_MAX;
    var dissimilar_max = 0.0;
    var mask = 1u << 4u;
    var idx = 0u;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            if (!(x == 0 && y == 0)) {
                let s = lock_luma(clamp_load(p, vec2<i32>(x, y), render_size_i()));
                let difference = max(s, nucleus) / min(s, nucleus);
                if (difference > 0.0 && difference < similar_threshold) {
                    mask |= 1u << idx;
                } else {
                    dissimilar_min = min(dissimilar_min, s);
                    dissimilar_max = max(dissimilar_max, s);
                }
            }
            idx += 1u;
        }
    }
    let is_ridge = nucleus > dissimilar_max || nucleus < dissimilar_min;
    if (!is_ridge) {
        return false;
    }
    // Upper left, upper right, lower left, lower right: a 2×2 of similar texels
    // around the centre means it is no thin feature.
    let quads = array<u32, 4>(0x1bu, 0x36u, 0xd8u, 0x1b0u);
    for (var i = 0; i < 4; i++) {
        if ((mask & quads[i]) == quads[i]) {
            return false;
        }
    }
    return true;
}

@compute @workgroup_size(8, 8)
fn cs_lock(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    let size = render_size_i();
    if (!is_on_screen(p, size)) {
        return;
    }
    if (thin_feature(p)) {
        let hr = compute_hr_pos_from_lr_pos(p);
        let display = display_size_i();
        if (is_on_screen(hr, display)) {
            new_locks[u32(hr.y) * u32(display.x) + u32(hr.x)] = 1u;
        }
    }
    // `ClearResourcesForNextFrame`: the far plane, never inverted.
    prev_depth[u32(p.y) * u32(size.x) + u32(p.x)] = bitcast<u32>(FAR_DEPTH);
}
