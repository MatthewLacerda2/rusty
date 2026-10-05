// assets/shaders/ssao.wgsl — screen-space ambient occlusion (#436).
//
// Two fullscreen passes over the SSAO depth prepass, run before the forward pass:
//  * `fs_ao`   — hemisphere occlusion at the tier's resolution (half on Medium, full
//                on High), world-space, from the prepass depth alone (normals are
//                rebuilt from neighbouring depths).
//  * `fs_blur` — a depth-aware 5x5 blur that also upsamples to full resolution, so
//                the forward shader reads one texel per pixel.
// The forward shader multiplies only its ambient/indirect terms by the result.

struct SsaoParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    camera_fwd: vec4<f32>,
    // x radius (world units), y intensity (exponent), z sample count,
    // w full-res pixels per AO texel (1 or 2)
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> p: SsaoParams;
@group(0) @binding(1)
var t_depth: texture_depth_2d;
// The raw occlusion `fs_blur` reads; a 1x1 placeholder while `fs_ao` runs.
@group(0) @binding(2)
var t_raw: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
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

// Distance along the camera's forward axis — the depth the range checks compare.
fn view_depth(world: vec3<f32>) -> f32 {
    return dot(world - p.camera_pos.xyz, p.camera_fwd.xyz);
}

// Interleaved gradient noise (Jimenez 2014): a per-pixel rotation the blur removes.
fn ign(px: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(px, vec2<f32>(0.06711056, 0.00583715))));
}

// Of the two one-sided differences along an axis, the one that stays on this
// surface (smaller depth step), so a silhouette never bends the normal.
fn flatter(back: vec3<f32>, fwd: vec3<f32>) -> vec3<f32> {
    let f = p.camera_fwd.xyz;
    if (dot(back, back) < 1e-12 || (dot(fwd, fwd) > 1e-12 && abs(dot(fwd, f)) < abs(dot(back, f)))) {
        return fwd;
    }
    return back;
}

// How many pixels a `radius` offset at `P`, across the view, spans on screen.
fn radius_in_pixels(P: vec3<f32>, radius: f32, dims: vec2<f32>) -> f32 {
    let f = p.camera_fwd.xyz;
    let side = normalize(cross(f, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(f.y) > 0.9)));
    let a = p.view_proj * vec4<f32>(P, 1.0);
    let b = p.view_proj * vec4<f32>(P + side * radius, 1.0);
    return length((b.xy / b.w - a.xy / a.w) * 0.5 * dims);
}

@fragment
fn fs_ao(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy) * step;
    let d = load_depth(px);
    if (d >= 1.0) {
        return vec4<f32>(1.0);
    }
    let P = world_at(px, d);
    let dx = flatter(P - world_px(px - vec2<i32>(step, 0)), world_px(px + vec2<i32>(step, 0)) - P);
    let dy = flatter(P - world_px(px - vec2<i32>(0, step)), world_px(px + vec2<i32>(0, step)) - P);
    let c = cross(dx, dy);
    if (dot(c, c) < 1e-20) {
        return vec4<f32>(1.0);
    }
    var N = normalize(c);
    if (dot(N, p.camera_pos.xyz - P) < 0.0) {
        N = -N;
    }
    let helper = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(N.x) > 0.9);
    let T = normalize(helper - N * dot(helper, N));
    let B = cross(N, T);

    let radius = p.params.x;
    let n = u32(p.params.z);
    let rot = ign(frag.xy) * 6.2831853;
    let jitter = ign(frag.yx + 17.0);
    let dims = vec2<f32>(textureDimensions(t_depth));
    let depth_p = view_depth(P);
    // Slope-scaled bias: a sample is snapped to a pixel centre, and at a grazing
    // angle one pixel spans a long stretch of the same surface, so allow one
    // pixel's worth of depth change along it before calling anything an occluder.
    let r_px = radius_in_pixels(P, radius, dims);
    let grazing = max(abs(dot(N, normalize(p.camera_pos.xyz - P))), 0.1);
    let bias = 0.02 * radius + radius / max(r_px, 1e-3) / grazing;
    var occlusion = 0.0;
    for (var i = 0u; i < n; i = i + 1u) {
        // Cosine-weighted hemisphere direction on a golden-angle spiral; the reach
        // is biased short so contact detail gets most of the samples.
        let t = (f32(i) + 0.5) / f32(n);
        let phi = f32(i) * 2.39996323 + rot;
        let sin_t = sqrt(t);
        let dir = T * (cos(phi) * sin_t) + B * (sin(phi) * sin_t) + N * sqrt(1.0 - t);
        let s = fract(f32(i) * 0.7548777 + jitter);
        let S = P + N * (0.02 * radius) + dir * (radius * mix(0.1, 1.0, s * s));
        let clip = p.view_proj * vec4<f32>(S, 1.0);
        if (clip.w <= 0.0) {
            continue;
        }
        let ndc = clip.xy / clip.w;
        let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if (any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0))) {
            continue;
        }
        let scene_d = view_depth(world_px(vec2<i32>(uv * dims)));
        // Fade occluders far outside the radius (a wall metres behind a pillar).
        let range = smoothstep(0.0, 1.0, radius / max(abs(depth_p - scene_d), 1e-4));
        if (scene_d < view_depth(S) - bias) {
            occlusion += range;
        }
    }
    // Where the radius covers under a couple of pixels (the far floor, the horizon)
    // pixel quantization alone reads as occlusion; fade AO out there.
    let fade = smoothstep(1.0, 3.0, r_px);
    let ao = 1.0 - fade * occlusion / f32(max(n, 1u));
    return vec4<f32>(pow(ao, p.params.y), 0.0, 0.0, 1.0);
}

@fragment
fn fs_blur(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let step = i32(p.params.w);
    let px = vec2<i32>(frag.xy);
    let d = load_depth(px);
    if (d >= 1.0) {
        return vec4<f32>(1.0);
    }
    let center = view_depth(world_at(px, d));
    // Taps whose depth differs by more than ~5% of the view distance fall away, so
    // occlusion never bleeds across a silhouette.
    let falloff = 0.05 * max(center, 0.1);
    let base = px / step;
    let raw_max = vec2<i32>(textureDimensions(t_raw)) - 1;
    var sum = 0.0;
    var weight = 0.0;
    for (var y = -2; y <= 2; y = y + 1) {
        for (var x = -2; x <= 2; x = x + 1) {
            let tap = clamp(base + vec2<i32>(x, y), vec2<i32>(0), raw_max);
            let w = exp(-abs(view_depth(world_px(tap * step)) - center) / falloff);
            sum += textureLoad(t_raw, tap, 0).r * w;
            weight += w;
        }
    }
    return vec4<f32>(sum / max(weight, 1e-4), 0.0, 0.0, 1.0);
}
