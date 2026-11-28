// SSAO over the view's depth, before the forward pass. AO texel `t` stands for full-resolution
// pixel `t * step`: every pass reads that pixel's depth for it.
#import fullscreen
#import depth
#import lighting::screen_space::{bilateral_blur, bilateral_upsample, hemisphere, ign, load_depth, normal_at, radius_in_pixels, screen_uv, slope_bias, tangents, View, view_depth, world_at, world_px}

struct SsaoParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    camera_fwd: vec4<f32>,
    // x radius (world units), y intensity (an exponent), z sample count, w full-res pixels
    // per AO texel.
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> p: SsaoParams;
@group(0) @binding(1)
var t_depth: texture_depth_2d;
// The occlusion a stage reads: `fs_ao`'s for the blur, the blur's for the upsample; `fs_ao`
// binds a placeholder.
@group(0) @binding(2)
var t_raw: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(i);
}

fn view() -> View {
    return View(p.inv_view_proj, p.view_proj, p.camera_pos.xyz, p.camera_fwd.xyz);
}

@fragment
fn fs_ao(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy) * step;
    let d = load_depth(t_depth, px);
    if (depth::is_sky(d)) {
        return vec4<f32>(1.0);
    }
    let v = view();
    let P = world_at(t_depth, v, px, d);
    let N = normal_at(t_depth, v, px, P, step);
    if (dot(N, N) == 0.0) {
        return vec4<f32>(1.0);
    }
    let tb = tangents(N);

    let radius = p.params.x;
    let n = u32(p.params.z);
    let rot = ign(frag.xy) * 6.2831853;
    let jitter = ign(frag.yx + 17.0);
    let dims = vec2<f32>(textureDimensions(t_depth));
    let depth_p = view_depth(v, P);
    let r_px = radius_in_pixels(v, P, radius, dims);
    // Slope-scaled (`slope_bias`): one pixel's worth of depth change along the surface is not
    // an occluder.
    let bias = slope_bias(v, P, N, radius, dims);
    var occlusion = 0.0;
    for (var i = 0u; i < n; i = i + 1u) {
        let dir = hemisphere(i, n, rot, N, tb);
        let s = fract(f32(i) * 0.7548777 + jitter);
        // Reach biased short (`s * s`, at least a tenth), so contact detail gets most samples.
        let S = P + N * (0.02 * radius) + dir * (radius * mix(0.1, 1.0, s * s));
        let uv = screen_uv(v, S);
        if (uv.x < 0.0) {
            continue;
        }
        let scene_d = view_depth(v, world_px(t_depth, v, vec2<i32>(uv * dims)));
        // Fades occluders far outside the radius (a wall metres behind a pillar).
        let range = smoothstep(0.0, 1.0, radius / max(abs(depth_p - scene_d), 1e-4));
        if (scene_d < view_depth(v, S) - bias) {
            occlusion += range;
        }
    }
    // Under a few pixels of radius (the far floor), quantization alone reads as occlusion.
    let fade = smoothstep(1.0, 3.0, r_px);
    let ao = 1.0 - fade * occlusion / f32(max(n, 1u));
    return vec4<f32>(pow(ao, p.params.y), 0.0, 0.0, 1.0);
}

@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let base = vec2<i32>(frag.xy);
    let d = load_depth(t_depth, base * step);
    if (depth::is_sky(d)) {
        return vec4<f32>(1.0);
    }
    let v = view();
    let center = view_depth(v, world_at(t_depth, v, base * step, d));
    let ao = bilateral_blur(t_depth, v, t_raw, base, step, 2, center).r;
    return vec4<f32>(ao, 0.0, 0.0, 1.0);
}

@fragment
fn fs_upsample(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy);
    let d = load_depth(t_depth, px);
    if (depth::is_sky(d)) {
        return vec4<f32>(1.0);
    }
    let v = view();
    let center = view_depth(v, world_at(t_depth, v, px, d));
    let ao = bilateral_upsample(t_depth, v, t_raw, px, step, center).r;
    return vec4<f32>(ao, 0.0, 0.0, 1.0);
}
