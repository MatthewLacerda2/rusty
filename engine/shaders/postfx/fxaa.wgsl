// engine/shaders/postfx/fxaa.wgsl — FXAA (#360), the post chain's finishing pass:
// the composite's (or the authored effects') LDR image in, the anti-aliased output out
// (`render::postfx::effects::fxaa`). It binds the chain's IO layout and reads only
// the colour (binding 1) and its sampler (binding 2).

#import fullscreen

@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;

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
// The texel size comes from `textureDimensions(t_color)`: the pass runs at full
// resolution.

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
