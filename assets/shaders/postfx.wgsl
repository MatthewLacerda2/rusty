// postfx.wgsl — fullscreen post-process chain.
//
// Three entry points share one fullscreen-triangle vertex stage:
//   fs_bright    — bright-pass threshold (bloom prefilter)
//   fs_blur      — separable gaussian blur (direction from uniform)
//   fs_composite — bloom add + SSR + motion blur + color correction + tonemap
//
// The composite is the one place the UI knobs (exposure/contrast/saturation/
// bloom/motion-blur/SSR) finally take effect.

struct PostParams {
    // x: exposure (EV), y: contrast, z: saturation, w: gamma
    color: vec4<f32>,
    // x: bloom_intensity, y: bloom_threshold, z: tonemap index, w: bloom_enabled
    bloom: vec4<f32>,
    // x: blur direction (0=horizontal,1=vertical), y: texel_x, z: texel_y, w: motion_blur_samples
    misc: vec4<f32>,
    // x: ssr_mode (0 off, 1 cubemap, 2 screen-space), y: motion_blur_active,
    // z: motion_blur_scale, w: unused
    flags: vec4<f32>,
    // current inverse view-projection (reconstruct world pos from depth)
    inv_view_proj: mat4x4<f32>,
    // previous view-projection (camera motion blur velocity)
    prev_view_proj: mat4x4<f32>,
    // current view-projection (SSR ray marching to clip space)
    view_proj: mat4x4<f32>,
    // xyz: camera world position, w: unused
    camera_pos: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: PostParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
@group(0) @binding(3) var t_bloom: texture_2d<f32>;
@group(0) @binding(4) var t_depth: texture_depth_2d;
@group(0) @binding(5) var t_skybox: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    // Oversized triangle covering the screen; UVs in [0,1].
    var out: VsOut;
    let x = f32((vid << 1u) & 2u);
    let y = f32(vid & 2u);
    out.uv = vec2<f32>(x, y);
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Point-load the depth target at a [0,1] uv (depth textures can't be sampled
// with a filtering sampler, so we convert uv -> integer texel and load).
fn load_depth(uv: vec2<f32>) -> f32 {
    let dim = vec2<f32>(textureDimensions(t_depth));
    let coord = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * dim);
    return textureLoad(t_depth, coord, 0);
}

// ---- Bright-pass (bloom prefilter) ----
@fragment
fn fs_bright(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(t_color, s_color, in.uv).rgb;
    let threshold = params.bloom.y;
    let l = luminance(c);
    let factor = max(l - threshold, 0.0) / max(l, 0.0001);
    return vec4<f32>(c * factor, 1.0);
}

// 1D gaussian weight by absolute offset (0..2). A function avoids dynamic array
// indexing, which naga forbids on function-local arrays.
fn gauss_w(i: i32) -> f32 {
    if (i == 0) { return 0.227027; }
    if (i == 1 || i == -1) { return 0.3162162; }
    return 0.0702703;
}

// ---- 2D gaussian blur (single pass; cheap at the reduced bloom resolution) ----
@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4<f32> {
    let tx = params.misc.y;
    let ty = params.misc.z;
    var result = vec3<f32>(0.0);
    for (var x = -2; x <= 2; x = x + 1) {
        for (var y = -2; y <= 2; y = y + 1) {
            let off = vec2<f32>(f32(x) * tx, f32(y) * ty);
            let weight = gauss_w(x) * gauss_w(y);
            result += textureSample(t_color, s_color, in.uv + off).rgb * weight;
        }
    }
    return vec4<f32>(result, 1.0);
}

// Reconstruct a surface normal from neighbouring depth via screen-space
// derivatives — avoids a full normal G-buffer for the SSR pass.
fn normal_from_depth(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dx = dpdx(world_pos);
    let dy = dpdy(world_pos);
    return normalize(cross(dx, dy));
}

fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = params.inv_view_proj * ndc;
    return world.xyz / world.w;
}

// Screen-space reflection (real SSR): march the reflected ray in world space,
// project each step to screen, and compare reconstructed depth. Falls back to
// the cubemap sample on miss so reflective surfaces never go black.
fn screen_space_reflection(uv: vec2<f32>, world_pos: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let v = normalize(world_pos - params.camera_pos.xyz);
    let r = reflect(v, n);
    var p = world_pos + r * 0.1;
    let step = 0.25;
    for (var i = 0; i < 24; i = i + 1) {
        p = p + r * step;
        let clip = params.view_proj * vec4<f32>(p, 1.0);
        if (clip.w <= 0.0) { break; }
        let s = clip.xyz / clip.w;
        let suv = vec2<f32>(s.x * 0.5 + 0.5, 1.0 - (s.y * 0.5 + 0.5));
        if (suv.x < 0.0 || suv.x > 1.0 || suv.y < 0.0 || suv.y > 1.0) { break; }
        let scene_depth = load_depth(suv);
        if (scene_depth < s.z - 0.0005 && s.z - scene_depth < 0.02) {
            return textureSample(t_color, s_color, suv).rgb;
        }
    }
    return cubemap_reflection(r);
}

fn cubemap_reflection(r: vec3<f32>) -> vec3<f32> {
    let pi = 3.14159265359;
    let phi = atan2(r.z, r.x);
    let theta = acos(clamp(r.y, -1.0, 1.0));
    let u = (phi + pi) / (2.0 * pi);
    let vv = theta / pi;
    return textureSample(t_skybox, s_color, vec2<f32>(u, vv)).rgb;
}

// Camera motion blur: blur along the screen-space velocity of the pixel between
// the previous and current view-projection. Driven by motion_blur_samples.
fn motion_blur(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let prev_clip = params.prev_view_proj * vec4<f32>(world_pos, 1.0);
    if (prev_clip.w <= 0.0) {
        return textureSample(t_color, s_color, uv).rgb;
    }
    let prev_ndc = prev_clip.xy / prev_clip.w;
    let prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 1.0 - (prev_ndc.y * 0.5 + 0.5));
    let velocity = (uv - prev_uv) * params.flags.z;
    let samples = i32(params.misc.w);
    var color = vec3<f32>(0.0);
    var total = 0.0;
    for (var i = 0; i < samples; i = i + 1) {
        let t = f32(i) / max(f32(samples - 1), 1.0) - 0.5;
        let suv = clamp(uv + velocity * t, vec2<f32>(0.0), vec2<f32>(1.0));
        color += textureSample(t_color, s_color, suv).rgb;
        total += 1.0;
    }
    return color / max(total, 1.0);
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap(c: vec3<f32>, mode: u32) -> vec3<f32> {
    if (mode == 1u) {
        return c / (c + vec3<f32>(1.0));
    } else if (mode == 2u) {
        return aces(c);
    }
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {
    var hdr: vec3<f32>;
    let depth = load_depth(in.uv);
    let world_pos = world_from_depth(in.uv, depth);

    // 1. Motion blur (samples the HDR scene along camera velocity).
    if (params.flags.y > 0.5 && params.misc.w > 1.5 && depth < 1.0) {
        hdr = motion_blur(in.uv, world_pos);
    } else {
        hdr = textureSample(t_color, s_color, in.uv).rgb;
    }

    // 2. Real screen-space reflections (High tier only; Low/Medium rely on the
    //    forward shader's cubemap reflection). Additive, grazing-weighted.
    if (params.flags.x > 1.5 && depth < 1.0) {
        let n = normal_from_depth(in.uv, world_pos);
        let refl = screen_space_reflection(in.uv, world_pos, n);
        let v = normalize(params.camera_pos.xyz - world_pos);
        let fresnel = pow(1.0 - max(dot(n, v), 0.0), 3.0);
        hdr += refl * (0.05 + fresnel * 0.3);
    }

    // 3. Bloom add.
    if (params.bloom.w > 0.5) {
        hdr += textureSample(t_bloom, s_color, in.uv).rgb * params.bloom.x;
    }

    // 4. Exposure.
    hdr *= exp2(params.color.x);

    // 5. Tonemap (HDR -> LDR).
    hdr = tonemap(hdr, u32(params.bloom.z));

    // 6. Contrast (around 0.5 midpoint).
    hdr = (hdr - vec3<f32>(0.5)) * params.color.y + vec3<f32>(0.5);

    // 7. Saturation.
    let l = luminance(hdr);
    hdr = mix(vec3<f32>(l), hdr, params.color.z);

    // 8. Gamma encode.
    hdr = clamp(hdr, vec3<f32>(0.0), vec3<f32>(1.0));
    hdr = pow(hdr, vec3<f32>(1.0 / max(params.color.w, 0.01)));

    return vec4<f32>(hdr, 1.0);
}
