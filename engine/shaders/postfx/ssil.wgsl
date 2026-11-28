// Godot 4's SSIL: one bounce of last frame's realtime light for the forward pass's
// indirect diffuse. `fs_source` is next frame's source: HDR colour times the bounce
// share the opaque pass wrote in alpha, block-averaged; the sky and a NaN add nothing.
// `fs_gather` reads the source through last frame's view-projection over a cosine-weighted
// hemisphere (SSAO's), where the surfaces face each other and nothing on screen stands
// between (`blocked`). Texel `t` is full-resolution pixel `t * step`.

#import fullscreen
#import depth
#import numeric::finite3
#import lighting::screen_space::{bilateral_blur, bilateral_upsample, hemisphere, ign, load_depth, normal_at, radius_in_pixels, screen_uv, slope_bias, tangents, View, view_depth, world_at, world_px}

struct SsilParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    camera_fwd: vec4<f32>,
    prev_camera_pos: vec4<f32>,
    // `ssil::SsilUniform` says what each `w` and these hold; the radius is in world units.
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> p: SsilParams;
@group(0) @binding(1)
var t_depth: texture_depth_2d;
@group(0) @binding(2)
var t_in: texture_2d<f32>;
@group(0) @binding(3)
var t_prev: texture_2d<f32>;
@group(0) @binding(4)
var t_prev_normal: texture_2d<f32>;

const HISTORY: f32 = 0.9;
// A share of the distance from the eye.
const REJECT: f32 = 0.05;
// The least cosine (about 25°) between a normal and its history's.
const REJECT_TURN: f32 = 0.9;
// Shares of the point-to-sample segment.
const OCCLUSION_TAPS: array<f32, 3> = array<f32, 3>(0.3, 0.5, 0.7);

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(i);
}

fn view() -> View {
    return View(p.inv_view_proj, p.view_proj, p.camera_pos.xyz, p.camera_fwd.xyz);
}

// Black when a channel is not finite or past `Rgba16Float`'s range: no NaN or infinity
// spreads.
fn finite(c: vec3<f32>) -> vec3<f32> {
    return select(vec3<f32>(0.0), c, finite3(c) && all(abs(c) <= vec3<f32>(65504.0)));
}

fn prev_uv(P: vec3<f32>) -> vec2<f32> {
    let prev = View(p.inv_view_proj, p.prev_view_proj, p.prev_camera_pos.xyz, p.camera_fwd.xyz);
    return screen_uv(prev, P);
}

@fragment
fn fs_source(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let base = vec2<i32>(frag.xy) * step;
    let last = vec2<i32>(textureDimensions(t_in)) - 1;
    var sum = vec3<f32>(0.0);
    var count = 0.0;
    for (var y = 0; y < step; y = y + 1) {
        for (var x = 0; x < step; x = x + 1) {
            let px = min(base + vec2<i32>(x, y), last);
            if (depth::is_sky(load_depth(t_depth, px))) {
                continue;
            }
            let c = textureLoad(t_in, px, 0);
            sum += finite(c.rgb * saturate(c.a));
            count += 1.0;
        }
    }
    return vec4<f32>(sum / max(count, 1.0), 1.0);
}

// A pillar metres in front of the segment hides it from the eye, not from `P`: hence
// `thickness`.
fn blocked(v: View, P: vec3<f32>, Q: vec3<f32>, dims: vec2<f32>, bias: f32, thickness: f32) -> bool {
    for (var k = 0; k < 3; k = k + 1) {
        let M = mix(P, Q, OCCLUSION_TAPS[k]);
        let uv = screen_uv(v, M);
        if (uv.x < 0.0) {
            continue;
        }
        let behind = view_depth(v, M) - view_depth(v, world_px(t_depth, v, vec2<i32>(uv * dims)));
        if (behind > bias && behind < thickness) {
            return true;
        }
    }
    return false;
}

@fragment
fn fs_gather(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy) * step;
    let d = load_depth(t_depth, px);
    if (p.camera_fwd.w < 0.5 || depth::is_sky(d)) {
        return vec4<f32>(0.0);
    }
    let v = view();
    let P = world_at(t_depth, v, px, d);
    let N = normal_at(t_depth, v, px, P, step);
    if (dot(N, N) == 0.0) {
        return vec4<f32>(0.0);
    }
    let tb = tangents(N);
    let radius = p.params.x;
    let n = u32(p.params.z);
    // IGN's temporal offset (Jimenez 2014): a still pattern left blotches the blur could not
    // remove.
    let turn = vec2<f32>(5.588238 * p.camera_pos.w);
    let rot = ign(frag.xy + turn) * 6.2831853;
    let jitter = ign(frag.yx + 17.0 + turn);
    let dims = vec2<f32>(textureDimensions(t_depth));
    let source = vec2<f32>(textureDimensions(t_in));
    let bias = slope_bias(v, P, N, radius, dims);
    // One pixel's depth step at the reach, not `slope_bias`: the taps sit in the open.
    let between = 0.02 * radius + radius / max(radius_in_pixels(v, P, radius, dims), 1e-3);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = fract(f32(i) * 0.7548777 + jitter);
        let S = P + N * (0.02 * radius) + hemisphere(i, n, rot, N, tb) * (radius * mix(0.1, 1.0, s));
        let uv = screen_uv(v, S);
        if (uv.x < 0.0) {
            continue;
        }
        let q = vec2<i32>(uv * dims);
        let dq = load_depth(t_depth, q);
        if (depth::is_sky(dq)) {
            continue;
        }
        let Q = world_at(t_depth, v, q, dq);
        let to_q = Q - P;
        let dist = length(to_q);
        let NQ = normal_at(t_depth, v, q, Q, 1);
        let facing = dot(N, to_q) > bias && dot(NQ, -to_q) > 0.0;
        // Fades out over the last third of 1.5 radii.
        let reach = saturate(3.0 - 2.0 * dist / radius);
        let was = prev_uv(Q);
        if (!facing || reach <= 0.0 || was.x < 0.0 || blocked(v, P, Q, dims, between, radius)) {
            continue;
        }
        let light = finite(textureLoad(t_in, vec2<i32>(was * source), 0).rgb);
        sum += light * reach;
    }
    return vec4<f32>(finite(sum * (p.params.y / f32(max(n, 1u)))), 1.0);
}

@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let base = vec2<i32>(frag.xy);
    let d = load_depth(t_depth, base * step);
    if (depth::is_sky(d)) {
        return vec4<f32>(0.0);
    }
    let v = view();
    let center = view_depth(v, world_at(t_depth, v, base * step, d));
    return vec4<f32>(bilateral_blur(t_depth, v, t_in, base, step, 3, center).rgb, 1.0);
}

struct Accumulated {
    @location(0) result: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

@fragment
fn fs_accumulate(@builtin(position) frag: vec4<f32>) -> Accumulated {
    let step = i32(p.params.w);
    let t = vec2<i32>(frag.xy);
    let d = load_depth(t_depth, t * step);
    if (depth::is_sky(d)) {
        return Accumulated(vec4<f32>(0.0), vec4<f32>(0.5, 0.5, 0.5, 1.0));
    }
    let v = view();
    let P = world_at(t_depth, v, t * step, d);
    let N = normal_at(t_depth, v, t * step, P, step);
    let normal = vec4<f32>(N * 0.5 + 0.5, 1.0);
    let dist = distance(P, p.camera_pos.xyz);
    let last = vec2<i32>(textureDimensions(t_in)) - 1;
    let now = finite(textureLoad(t_in, t, 0).rgb);
    let was = prev_uv(P);
    if (p.camera_fwd.w < 0.5 || was.x < 0.0) {
        return Accumulated(vec4<f32>(now, dist), normal);
    }
    let at = vec2<i32>(was * vec2<f32>(last + 1));
    let history = textureLoad(t_prev, at, 0);
    let moved = abs(distance(P, p.prev_camera_pos.xyz) - history.a);
    let turned = dot(N, textureLoad(t_prev_normal, at, 0).xyz * 2.0 - 1.0) < REJECT_TURN;
    if (moved > REJECT * dist || turned) {
        return Accumulated(vec4<f32>(now, dist), normal);
    }
    // A light switched off, or a surface uncovered behind a mover, never lingers.
    var lo = now;
    var hi = now;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let c = finite(textureLoad(t_in, clamp(t + vec2<i32>(x, y), vec2<i32>(0), last), 0).rgb);
            lo = min(lo, c);
            hi = max(hi, c);
        }
    }
    let kept = clamp(finite(history.rgb), lo, hi);
    return Accumulated(vec4<f32>(mix(now, kept, HISTORY), dist), normal);
}

@fragment
fn fs_upsample(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy);
    let d = load_depth(t_depth, px);
    if (depth::is_sky(d)) {
        return vec4<f32>(0.0);
    }
    let v = view();
    let center = view_depth(v, world_at(t_depth, v, px, d));
    return vec4<f32>(bilateral_upsample(t_depth, v, t_in, px, step, center).rgb, 1.0);
}
