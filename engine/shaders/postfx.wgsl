// postfx.wgsl — fullscreen post-process chain.
//
// Four entry points share one fullscreen-triangle vertex stage:
//   fs_bright    — bright-pass threshold (bloom prefilter)
//   fs_blur      — separable gaussian blur (direction from uniform)
//   fs_composite — bloom add + SSR + exposure, then the grading LUT, over the scene
//                  (already motion-blurred by `postfx/motion_blur.wgsl` when it runs)
//   fs_fxaa      — luma-based edge anti-aliasing; the last pass of all (#360)
//
// The composite is the one place the UI knobs (exposure, bloom, SSR) finally take
// effect; the grade and the tonemap reach it baked into the grading LUT
// (`postfx/grade.wgsl`, #1440).

// `PostParams` lives in `common.wgsl` so authored postfx effects (#397) can bind it
// too (binding 0) — they read the game time from it (#398).
#import common::{PostParams, equirect_uv}
#import fullscreen
#import postfx::grade::{luminance, lut_coord}

@group(0) @binding(0) var<uniform> params: PostParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
@group(0) @binding(3) var t_bloom: texture_2d<f32>;
@group(0) @binding(4) var t_depth: texture_depth_2d;
@group(0) @binding(5) var t_skybox: texture_2d<f32>;
// The panorama's own sampler (wrapping), not `s_color`'s clamp: the SSR fallback's
// seam at φ = ±π must wrap as the skybox pass's does (#995).
@group(0) @binding(6) var s_skybox: sampler;
// The view's grading LUT (#1440): grade + tonemap, indexed by `lut_coord`.
@group(0) @binding(7) var t_lut: texture_3d<f32>;
// The view's adapted exposure (#1444): x is the EV, metered by `postfx/exposure.wgsl`.
@group(0) @binding(8) var<uniform> adapted: vec4<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    // Oversized triangle covering the screen; UVs in [0,1].
    var out: VsOut;
    out.uv = fullscreen::uv(vid);
    out.pos = fullscreen::triangle_down(vid);
    return out;
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
            // textureSampleLevel (explicit LOD 0), not textureSample: a gradient
            // (implicit-LOD) instruction inside a loop fails to compile on D3D12
            // (FXC "gradient instruction used in a loop"). These full-screen
            // targets have no mips, so LOD 0 is identical on every backend.
            result += textureSampleLevel(t_color, s_color, in.uv + off, 0.0).rgb * weight;
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
            // Explicit-LOD sample: a gradient instruction inside this break-bounded
            // (varying-iteration) loop fails to compile under D3D12/FXC.
            return textureSampleLevel(t_color, s_color, suv, 0.0).rgb;
        }
    }
    return sky_reflection(r);
}

fn sky_reflection(r: vec3<f32>) -> vec3<f32> {
    // Level 0, as the skybox pass reads it: the atan2 seam's derivatives would
    // pick a coarse mip (#1322).
    return textureSampleLevel(t_skybox, s_skybox, equirect_uv(r), 0.0).rgb;
}

@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {
    let depth = load_depth(in.uv);
    let world_pos = world_from_depth(in.uv, depth);

    // 1. The scene, motion-blurred upstream when the view runs it (#1436).
    var hdr = textureSample(t_color, s_color, in.uv).rgb;

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

    // 4. Exposure, the one colour knob outside the LUT: a pre-grade HDR scale, the
    //    volume's fixed EV or, in Automatic, the adapted one (#1444).
    hdr *= exp2(select(params.color.x, adapted.x, params.flags.w > 0.5));

    // 5. Grade + tonemap (HDR -> display): one trilinear fetch of the LUT.
    hdr = textureSampleLevel(t_lut, s_color, lut_coord(hdr), 0.0).rgb;

    return vec4<f32>(hdr, 1.0);
}

// ---------------------------------------------------------------------------
// FXAA (#360) — the final pass, run on the composite's output.
//
// FXAA 3.11's luma-based edge blend. It is defined on *post-tonemap, gamma-encoded*
// colour, which is why it runs after `fs_composite` rather than inside it: it needs
// the finished LDR image, and the neighbourhood taps below are only meaningful once
// every pixel has been tonemapped. The LDR target is sRGB (#415), so its taps come
// back *linear*; `fxaa_luma` takes the sqrt as the usual cheap re-encode, so the
// edge thresholds still see perceptual luma while the blend itself runs in linear.
//
// The texel size comes from `textureDimensions(t_color)` rather than the params
// uniform, because `misc.yz` is the blur pass's texel size and this pass runs at full
// resolution while the blur runs at the bloom divisor's.

// FXAA's own luma weights (NTSC), deliberately not `luminance()`'s Rec.709 ones:
// the algorithm's thresholds were tuned against these — on gamma-encoded colour.
fn fxaa_luma(c: vec3<f32>) -> f32 {
    return dot(sqrt(max(c, vec3<f32>(0.0))), vec3<f32>(0.299, 0.587, 0.114));
}

@fragment
fn fs_fxaa(in: VsOut) -> @location(0) vec4<f32> {
    // Tuning constants, at FXAA 3.11's defaults: how far along an edge to search,
    // and the floor that stops near-flat regions from resolving a direction out of
    // numerical noise.
    let span_max = 8.0;
    let reduce_mul = 1.0 / 8.0;
    let reduce_min = 1.0 / 128.0;

    let inv = vec2<f32>(1.0) / vec2<f32>(textureDimensions(t_color));

    let nw = textureSample(t_color, s_color, in.uv + vec2<f32>(-1.0, -1.0) * inv).rgb;
    let ne = textureSample(t_color, s_color, in.uv + vec2<f32>(1.0, -1.0) * inv).rgb;
    let sw = textureSample(t_color, s_color, in.uv + vec2<f32>(-1.0, 1.0) * inv).rgb;
    let se = textureSample(t_color, s_color, in.uv + vec2<f32>(1.0, 1.0) * inv).rgb;
    let m = textureSample(t_color, s_color, in.uv).rgb;

    let l_nw = fxaa_luma(nw);
    let l_ne = fxaa_luma(ne);
    let l_sw = fxaa_luma(sw);
    let l_se = fxaa_luma(se);
    let l_m = fxaa_luma(m);

    let luma_min = min(l_m, min(min(l_nw, l_ne), min(l_sw, l_se)));
    let luma_max = max(l_m, max(max(l_nw, l_ne), max(l_sw, l_se)));

    // The blur direction is perpendicular to the local luma gradient, i.e. *along*
    // the edge — which is what turns a staircase into a ramp.
    var dir = vec2<f32>(
        -((l_nw + l_ne) - (l_sw + l_se)),
        ((l_nw + l_sw) - (l_ne + l_se)),
    );
    let dir_reduce = max((l_nw + l_ne + l_sw + l_se) * 0.25 * reduce_mul, reduce_min);
    let rcp_dir_min = 1.0 / (min(abs(dir.x), abs(dir.y)) + dir_reduce);
    dir = clamp(
        dir * rcp_dir_min,
        vec2<f32>(-span_max),
        vec2<f32>(span_max),
    ) * inv;

    // Two taps for the narrow average, four for the wide one.
    let rgb_a = 0.5 * (textureSample(t_color, s_color, in.uv + dir * (1.0 / 3.0 - 0.5)).rgb
        + textureSample(t_color, s_color, in.uv + dir * (2.0 / 3.0 - 0.5)).rgb);
    let rgb_b = rgb_a * 0.5
        + 0.25 * (textureSample(t_color, s_color, in.uv + dir * -0.5).rgb
            + textureSample(t_color, s_color, in.uv + dir * 0.5).rgb);

    // The wide average is used only when it stays inside the neighbourhood's luma
    // range; outside it, the wide tap has strayed off the edge and would smear.
    let l_b = fxaa_luma(rgb_b);
    if (l_b < luma_min || l_b > luma_max) {
        return vec4<f32>(rgb_a, 1.0);
    }
    return vec4<f32>(rgb_b, 1.0);
}

// Plain copy (#397): lands the authored-effect chain's result in the output when FXAA
// is off. The chain ends in an internal target rather than the output so that result
// can also be kept as next frame's history (binding 3 of an authored effect).
@fragment
fn fs_copy(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(t_color, s_color, in.uv).rgb, 1.0);
}
