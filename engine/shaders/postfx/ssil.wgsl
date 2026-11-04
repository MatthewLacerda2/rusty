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
//                      source: that light, where the two surfaces face each other
//                      past SSAO's slope-scaled bias and nothing on screen stands
//                      between them (`blocked`), fading out by distance. Nothing
//                      before a history.
//  * `fs_blur`       — a depth-aware 7x7 blur.
//  * `fs_accumulate` — the temporal accumulate: last frame's result, reprojected
//                      through the camera and rejected where the surface's distance
//                      from the eye moved (a disocclusion) or its normal turned,
//                      clamped to this frame's neighbourhood and blended in at 90%.
//                      Its alpha holds the distance and its second target the
//                      normal, for next frame's rejection.
//  * `fs_upsample`   — below full resolution, a depth-aware bilinear upsample to it
//                      (SSAO's), so the forward pass reads one texel per pixel.
// Texel `t` stands for full-resolution pixel `t * step`, as in `ssao.wgsl`.

#import fullscreen
#import lighting::screen_space::{bilateral_blur, bilateral_upsample, hemisphere, ign, load_depth, normal_at, radius_in_pixels, screen_uv, slope_bias, tangents, View, view_depth, world_at, world_px}

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
// The normals last frame's accumulate kept (`n * 0.5 + 0.5`), for `fs_accumulate`.
@group(0) @binding(4)
var t_prev_normal: texture_2d<f32>;

// How fast the accumulate forgets: the history's weight where it is accepted.
const HISTORY: f32 = 0.9;
// How far, as a share of the distance from the eye, a reprojected surface may have
// moved before the history there is rejected.
const REJECT: f32 = 0.05;
// The least cosine between a surface's normal now and the one its history was
// gathered for (about 25 degrees): a surface turned further starts over.
const REJECT_TURN: f32 = 0.9;
// Where along the segment between a point and a sample's surface the gather looks
// for something on screen in between (`blocked`).
const OCCLUSION_TAPS: array<f32, 3> = array<f32, 3>(0.3, 0.5, 0.7);

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(i);
}

// The camera this frame's depth is read through (`lighting::screen_space`).
fn view() -> View {
    return View(p.inv_view_proj, p.view_proj, p.camera_pos.xyz, p.camera_fwd.xyz);
}

// `c` with every non-finite channel zeroed: a NaN or an infinity never spreads.
// Checked on the exponent bits (all ones), as `motion_blur.wgsl` and
// `ssr_pyramid.wgsl` do, since a compiler may fold a NaN compare away.
fn finite(c: vec3<f32>) -> vec3<f32> {
    let exponent = bitcast<vec3<u32>>(c) & vec3<u32>(0x7f800000u);
    let ok = (exponent != vec3<u32>(0x7f800000u)) & (abs(c) <= vec3<f32>(65504.0));
    return select(vec3<f32>(0.0), c, ok);
}

// Where world point `P` was in last frame's screen, in [0, 1]; x < 0 off it.
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
            if (load_depth(t_depth, px) >= 1.0) {
                continue;
            }
            let c = textureLoad(t_in, px, 0);
            sum += finite(c.rgb * saturate(c.a));
            count += 1.0;
        }
    }
    return vec4<f32>(sum / max(count, 1.0), 1.0);
}

// Whether something on screen stands between surface point `P` and the surface
// point `Q` a sample landed on: at each of `OCCLUSION_TAPS` along the segment, the
// surface on screen there is nearer the eye than the segment by more than `bias`,
// and by no more than `thickness` (a pillar metres in front of the segment hides
// it from the eye, not from `P`).
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
    if (p.camera_fwd.w < 0.5 || d >= 1.0) {
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
    // IGN's temporal offset (Jimenez 2014): each frame turns the pattern, so the
    // accumulate averages many frames' samples. A still pattern left blotches the
    // 8 samples and the blur could not remove (seen on a capsule's shadowed side).
    let turn = vec2<f32>(5.588238 * p.camera_pos.w);
    let rot = ign(frag.xy + turn) * 6.2831853;
    let jitter = ign(frag.yx + 17.0 + turn);
    let dims = vec2<f32>(textureDimensions(t_depth));
    let source = vec2<f32>(textureDimensions(t_in));
    // SSAO's slope-scaled bias: a sample's surface must stand off this point's
    // plane by more than depth noise, so a flat floor never lights itself.
    let bias = slope_bias(v, P, N, radius, dims);
    // What counts as standing in between: past one pixel's depth step at the
    // reach (no grazing scale: the taps sit in the open, off any surface).
    let between = 0.02 * radius + radius / max(radius_in_pixels(v, P, radius, dims), 1e-3);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < n; i = i + 1u) {
        let s = fract(f32(i) * 0.7548777 + jitter);
        let S = P + N * (0.02 * radius) + hemisphere(i, n, rot, N, tb) * (radius * mix(0.1, 1.0, s));
        let uv = screen_uv(v, S);
        if (uv.x < 0.0) {
            continue;
        }
        // The surface on screen where the sample lands, and whether it and this
        // point face each other.
        let q = vec2<i32>(uv * dims);
        let dq = load_depth(t_depth, q);
        if (dq >= 1.0) {
            continue;
        }
        let Q = world_at(t_depth, v, q, dq);
        let to_q = Q - P;
        let dist = length(to_q);
        let NQ = normal_at(t_depth, v, q, Q, 1);
        // Its light reaches this point only where the two face each other, past
        // the bias; it fades out over the last third of 1.5 radii.
        let facing = dot(N, to_q) > bias && dot(NQ, -to_q) > 0.0;
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
    if (d >= 1.0) {
        return vec4<f32>(0.0);
    }
    let v = view();
    let center = view_depth(v, world_at(t_depth, v, base * step, d));
    return vec4<f32>(bilateral_blur(t_depth, v, t_in, base, step, 3, center).rgb, 1.0);
}

struct Accumulated {
    // rgb the bounce, a the surface's distance from the eye.
    @location(0) result: vec4<f32>,
    // The surface's normal, as `n * 0.5 + 0.5`.
    @location(1) normal: vec4<f32>,
};

@fragment
fn fs_accumulate(@builtin(position) frag: vec4<f32>) -> Accumulated {
    let step = i32(p.params.w);
    let t = vec2<i32>(frag.xy);
    let d = load_depth(t_depth, t * step);
    if (d >= 1.0) {
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
    return Accumulated(vec4<f32>(mix(now, kept, HISTORY), dist), normal);
}

// Full resolution from the result, SSAO's bilateral upsample on colour.
@fragment
fn fs_upsample(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy);
    let d = load_depth(t_depth, px);
    if (d >= 1.0) {
        return vec4<f32>(0.0);
    }
    let v = view();
    let center = view_depth(v, world_at(t_depth, v, px, d));
    return vec4<f32>(bilateral_upsample(t_depth, v, t_in, px, step, center).rgb, 1.0);
}
