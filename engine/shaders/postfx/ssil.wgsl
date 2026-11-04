// engine/shaders/postfx/ssil.wgsl — screen-space indirect lighting (#1750), Godot 4's
// SSIL: one bounce of last frame's realtime light off the surfaces on screen, for
// the forward pass's indirect diffuse (`render::postfx::effects::ssil`).
//
// Fullscreen passes, at the gather's resolution unless named:
//  * `fs_source`     — after the base camera's opaque pass: its HDR colour times the
//                      share of it the gather may bounce (the alpha the opaque
//                      pass wrote, `forward::lighting::with_ssil_share`), averaged over
//                      each texel's block of pixels. The sky and a NaN add nothing.
//                      This is next frame's source.
//  * `fs_gather`     — before the forward pass, over the SSAO prepass's depth: a
//                      cosine-weighted hemisphere of samples around each surface
//                      point (SSAO's pattern, `lighting::screen_space`). Each lands
//                      on the surface on screen there, is turned back into last
//                      frame's screen through its view-projection and reads the
//                      source: that light, where the two surfaces face each
//                      other, fading out by distance. Nothing before a history.
//  * `fs_blur`       — a depth-aware 7x7 blur.
//  * `fs_accumulate` — the temporal accumulate: last frame's result, reprojected
//                      through the camera and rejected where the surface's distance
//                      from the eye moved (a disocclusion), clamped to this frame's
//                      neighbourhood and blended in at 90%. Its alpha holds the
//                      distance, for next frame's rejection.
//  * `fs_upsample`   — below full resolution, a depth-aware bilinear upsample to it
//                      (SSAO's), so the forward pass reads one texel per pixel.
// Texel `t` stands for full-resolution pixel `t * step`, as in `ssao.wgsl`.

#import fullscreen
#import lighting::screen_space::{depth_weight, flatter, hemisphere, ign, tangents}

struct SsilParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    // The view-projection last frame's source and result were drawn with.
    prev_view_proj: mat4x4<f32>,
    // w: the frame's index (0..64), which turns the noise each frame.
    camera_pos: vec4<f32>,
    // w: 1 when the history holds last frame, 0 on a cut.
    camera_fwd: vec4<f32>,
    // xyz: last frame's eye.
    prev_camera_pos: vec4<f32>,
    // x radius (world units), y intensity, z sample count, w full-res pixels per texel
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> p: SsilParams;
@group(0) @binding(1)
var t_depth: texture_depth_2d;
// What a stage reads: the scene colour (`fs_source`), the source (`fs_gather`), the
// gather (`fs_blur`), the blur (`fs_accumulate`), the result (`fs_upsample`).
@group(0) @binding(2)
var t_in: texture_2d<f32>;
// Last frame's result, for `fs_accumulate`; the same as `t_in` for the others.
@group(0) @binding(3)
var t_prev: texture_2d<f32>;

// How fast the accumulate forgets: the history's weight where it is accepted.
const HISTORY: f32 = 0.9;
// How far, as a share of the distance from the eye, a reprojected surface may have
// moved before the history there is rejected.
const REJECT: f32 = 0.05;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(i);
}

fn load_depth(px: vec2<i32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(t_depth));
    return textureLoad(t_depth, clamp(px, vec2<i32>(0), dims - 1), 0);
}

// World position of full-resolution pixel `px` at NDC depth `d`.
fn world_at(px: vec2<i32>, d: f32) -> vec3<f32> {
    let uv = (vec2<f32>(px) + 0.5) / vec2<f32>(textureDimensions(t_depth));
    let w = p.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    return w.xyz / w.w;
}

fn world_px(px: vec2<i32>) -> vec3<f32> {
    return world_at(px, load_depth(px));
}

// Distance along the camera's forward axis, the depth the blurs compare.
fn view_depth(world: vec3<f32>) -> f32 {
    return dot(world - p.camera_pos.xyz, p.camera_fwd.xyz);
}

// The camera-facing normal at full-resolution pixel `px` (world `P`), rebuilt from
// the depths `step` pixels around it, as SSAO's; zero where they are degenerate.
fn normal_at(px: vec2<i32>, P: vec3<f32>, step: i32) -> vec3<f32> {
    let f = p.camera_fwd.xyz;
    let dx = flatter(P - world_px(px - vec2<i32>(step, 0)), world_px(px + vec2<i32>(step, 0)) - P, f);
    let dy = flatter(P - world_px(px - vec2<i32>(0, step)), world_px(px + vec2<i32>(0, step)) - P, f);
    let c = cross(dx, dy);
    if (dot(c, c) < 1e-20) {
        return vec3<f32>(0.0);
    }
    let N = normalize(c);
    return select(N, -N, dot(N, p.camera_pos.xyz - P) < 0.0);
}

// `c` with every non-finite channel zeroed: a NaN or an infinity never spreads.
fn finite(c: vec3<f32>) -> vec3<f32> {
    let ok = (c == c) & (abs(c) <= vec3<f32>(65504.0));
    return select(vec3<f32>(0.0), c, ok);
}

// Where world point `P` was in last frame's screen, in [0, 1]; `None` (x < 0) off it.
fn prev_uv(P: vec3<f32>) -> vec2<f32> {
    let clip = p.prev_view_proj * vec4<f32>(P, 1.0);
    if (clip.w <= 0.0) {
        return vec2<f32>(-1.0);
    }
    let ndc = clip.xy / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0))) {
        return vec2<f32>(-1.0);
    }
    return uv;
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
            if (load_depth(px) >= 1.0) {
                continue;
            }
            let c = textureLoad(t_in, px, 0);
            sum += finite(c.rgb * saturate(c.a));
            count += 1.0;
        }
    }
    return vec4<f32>(sum / max(count, 1.0), 1.0);
}

@fragment
fn fs_gather(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy) * step;
    let d = load_depth(px);
    if (p.camera_fwd.w < 0.5 || d >= 1.0) {
        return vec4<f32>(0.0);
    }
    let P = world_at(px, d);
    let N = normal_at(px, P, step);
    if (dot(N, N) == 0.0) {
        return vec4<f32>(0.0);
    }
    let tb = tangents(N);
    let radius = p.params.x;
    let n = u32(p.params.z);
    // IGN's temporal offset (Jimenez 2014): each frame turns the pattern, so the
    // accumulate averages many frames' samples. A still pattern left blotches the
    // 8 samples and the blur could not remove (seen on a capsule's shadowed side).
    let turn = vec2<f32>(5.588238 * p.camera_pos.w);
    let rot = ign(frag.xy + turn) * 6.2831853;
    let jitter = ign(frag.yx + 17.0 + turn);
    let dims = vec2<f32>(textureDimensions(t_depth));
    let source = vec2<f32>(textureDimensions(t_in));
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = fract(f32(i) * 0.7548777 + jitter);
        let S = P + N * (0.02 * radius) + hemisphere(i, n, rot, N, tb) * (radius * mix(0.1, 1.0, s));
        let clip = p.view_proj * vec4<f32>(S, 1.0);
        if (clip.w <= 0.0) {
            continue;
        }
        let ndc = clip.xy / clip.w;
        let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if (any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0))) {
            continue;
        }
        // The surface on screen where the sample lands, and whether it and this
        // point face each other.
        let q = vec2<i32>(uv * dims);
        let dq = load_depth(q);
        if (dq >= 1.0) {
            continue;
        }
        let Q = world_at(q, dq);
        let to_q = Q - P;
        let dist = length(to_q);
        let L = to_q / max(dist, 1e-4);
        let NQ = normal_at(q, Q, 1);
        // Its light reaches this point only where the two face each other; it
        // fades out over the last third of 1.5 radii.
        let facing = dot(NQ, -L) > 0.0 && dot(N, L) > 1e-4;
        let reach = saturate(3.0 - 2.0 * dist / radius);
        let was = prev_uv(Q);
        if (!facing || reach <= 0.0 || was.x < 0.0) {
            continue;
        }
        let light = finite(textureLoad(t_in, vec2<i32>(was * source), 0).rgb);
        sum += light * reach;
    }
    return vec4<f32>(finite(sum * (p.params.y / f32(max(n, 1u)))), 1.0);
}

// View depth at texel `t`: its full-resolution pixel's.
fn texel_depth(t: vec2<i32>, step: i32) -> f32 {
    return view_depth(world_px(t * step));
}

@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let base = vec2<i32>(frag.xy);
    let d = load_depth(base * step);
    if (d >= 1.0) {
        return vec4<f32>(0.0);
    }
    let center = view_depth(world_at(base * step, d));
    let last = vec2<i32>(textureDimensions(t_in)) - 1;
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    for (var y = -3; y <= 3; y = y + 1) {
        for (var x = -3; x <= 3; x = x + 1) {
            let tap = clamp(base + vec2<i32>(x, y), vec2<i32>(0), last);
            let w = depth_weight(texel_depth(tap, step), center);
            sum += textureLoad(t_in, tap, 0).rgb * w;
            weight += w;
        }
    }
    return vec4<f32>(sum / max(weight, 1e-4), 1.0);
}

@fragment
fn fs_accumulate(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let t = vec2<i32>(frag.xy);
    let d = load_depth(t * step);
    if (d >= 1.0) {
        return vec4<f32>(0.0);
    }
    let P = world_at(t * step, d);
    let dist = distance(P, p.camera_pos.xyz);
    let last = vec2<i32>(textureDimensions(t_in)) - 1;
    let now = finite(textureLoad(t_in, t, 0).rgb);
    let was = prev_uv(P);
    if (p.camera_fwd.w < 0.5 || was.x < 0.0) {
        return vec4<f32>(now, dist);
    }
    let history = textureLoad(t_prev, vec2<i32>(was * vec2<f32>(last + 1)), 0);
    let moved = abs(distance(P, p.prev_camera_pos.xyz) - history.a);
    if (moved > REJECT * dist) {
        return vec4<f32>(now, dist);
    }
    // This frame's 3x3 neighbourhood bounds the history: a light switched off, or a
    // surface uncovered behind a mover, never lingers.
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
    return vec4<f32>(mix(now, kept, HISTORY), dist);
}

// Full resolution from the result, SSAO's bilateral upsample on colour.
@fragment
fn fs_upsample(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy);
    let d = load_depth(px);
    if (d >= 1.0) {
        return vec4<f32>(0.0);
    }
    let center = view_depth(world_at(px, d));
    let f = vec2<f32>(px) / f32(step);
    let t0 = vec2<i32>(floor(f));
    let fr = f - floor(f);
    let last = vec2<i32>(textureDimensions(t_in)) - 1;
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    var nearest = vec3<f32>(0.0);
    var nearest_gap = 3.4e38;
    for (var i = 0; i < 4; i = i + 1) {
        let o = vec2<i32>(i & 1, i >> 1u);
        let tap = min(t0 + o, last);
        let td = texel_depth(tap, step);
        let c = textureLoad(t_in, tap, 0).rgb;
        let bw = mix(1.0 - fr, fr, vec2<f32>(o));
        let w = bw.x * bw.y * depth_weight(td, center);
        sum += c * w;
        weight += w;
        let gap = abs(td - center);
        if (gap < nearest_gap) {
            nearest_gap = gap;
            nearest = c;
        }
    }
    if (weight < 1e-3) {
        return vec4<f32>(nearest, 1.0);
    }
    return vec4<f32>(sum / weight, 1.0);
}
