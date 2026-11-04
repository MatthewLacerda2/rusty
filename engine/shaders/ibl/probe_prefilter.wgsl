// engine/shaders/ibl/probe_prefilter.wgsl — a reflection probe's GGX prefilter on the
// GPU (#1602). One dispatch per rough mip, one invocation per texel of every face:
// the texel's direction is the normal, and the captured cube's source mip chain
// (linear HDR, `reflection_prefilter::source::source_chain`) is convolved around it for
// roughness `mip / (mips - 1)` with the sampling the sky cube shares (`ibl::ggx`), each
// sample read from the source mip its solid angle picks (filtered importance sampling,
// Křivánek & Colbert 2008, #1696).
//
// It is `render::ibl::reflection_prefilter::cpu::prefilter` texel for texel, which stays
// the reference its tests pin: the same face matrices (`cubemap_capture::face_view_projection`,
// the stored layout's own, so it needs no second convention), the same per-sample mip,
// nearest-texel lookup and N·L weighting. The result is stored unclamped into an
// `Rgba16Float` face, read back for the KTX2 encode.

#import common::PI
#import ibl::ggx::{ggx_light, tangent_frame}

struct Params {
    // x: roughness, y: this mip's face size, z: the capture's face size,
    // w: the importance sample count (`reflection_prefilter::SAMPLE_COUNT`).
    p: vec4<f32>,
    // `cube_sample::FaceProjections`: each face camera's view-projection and inverse.
    view_proj: array<mat4x4<f32>, 6>,
    inv_view_proj: array<mat4x4<f32>, 6>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var t_capture: texture_2d_array<f32>;
@group(0) @binding(2) var out_face: texture_storage_2d_array<rgba16float, write>;

// World direction through face coords `uv` (clip-space NDC, v up) of `face`.
fn direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let world = params.inv_view_proj[face] * vec4<f32>(uv, 1.0, 1.0);
    return normalize(world.xyz / world.w);
}

// The captured radiance along `dir` in source mip `level` (base `params.p.z` wide): the
// nearest texel of the first face whose frustum holds it, as `cube_sample::sample`.
fn capture_at(dir: vec3<f32>, level: i32) -> vec3<f32> {
    let res = max(floor(params.p.z / exp2(f32(level))), 1.0);
    for (var face = 0u; face < 6u; face = face + 1u) {
        let clip = params.view_proj[face] * vec4<f32>(dir, 0.0);
        if (clip.w <= 0.0) {
            continue;
        }
        let ndc = clip.xy / clip.w;
        if (abs(ndc.x) <= 1.0 && abs(ndc.y) <= 1.0) {
            let last = i32(res) - 1;
            let px = clamp(i32(floor((ndc.x * 0.5 + 0.5) * res)), 0, last);
            let py = clamp(i32(floor((1.0 - (ndc.y * 0.5 + 0.5)) * res)), 0, last);
            return textureLoad(t_capture, vec2<i32>(px, py), i32(face), level).rgb;
        }
    }
    return vec3<f32>(0.0);
}

// The source chain at fractional level `lod`, as `cube_sample::sample_lod`: the two
// levels around it, blended.
fn capture(dir: vec3<f32>, lod: f32) -> vec3<f32> {
    let last = i32(textureNumLevels(t_capture)) - 1;
    let lo = min(i32(floor(lod)), last);
    let hi = min(lo + 1, last);
    return mix(capture_at(dir, lo), capture_at(dir, hi), lod - f32(lo));
}

// The source mip a sample with half-vector cosine `ndoth` reads, as
// `reflection_prefilter::source::sample_mip`.
fn sample_mip(ndoth: f32, a2: f32, samples: u32) -> f32 {
    let base = params.p.z;
    let d = ndoth * ndoth * (a2 - 1.0) + 1.0;
    let ggx = a2 / (PI * d * d);
    let omega_s = 4.0 / (f32(samples) * ggx);
    let omega_p = 4.0 * PI / (6.0 * base * base);
    let last = f32(textureNumLevels(t_capture) - 1u);
    return clamp(0.5 * log2(omega_s / omega_p) + 1.0, 0.0, last);
}

// GGX split-sum convolution around `n`, as `reflection_prefilter::cpu::convolve`.
fn convolve(n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let frame = tangent_frame(n);
    let a = roughness * roughness;
    let samples = u32(params.p.w);
    var color = vec3<f32>(0.0);
    var weight = 0.0;
    for (var i = 0u; i < samples; i = i + 1u) {
        let tap = ggx_light(i, samples, a * a, frame);
        let l = tap.xyz;
        let ndotl = dot(n, l);
        if (ndotl > 0.0) {
            color += capture(l, sample_mip(tap.w, a * a, samples)) * ndotl;
            weight += ndotl;
        }
    }
    if (weight > 0.0) {
        return color / weight;
    }
    return capture(n, 0.0);
}

@compute @workgroup_size(8, 8, 1)
fn cs_prefilter(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = u32(params.p.y);
    if (id.x >= size || id.y >= size) {
        return;
    }
    // `cube_sample::face_uv`: the texel centre, u right and v up.
    let centre = (vec2<f32>(id.xy) + 0.5) / f32(size);
    let uv = vec2<f32>(centre.x * 2.0 - 1.0, 1.0 - centre.y * 2.0);
    let color = convolve(direction(id.z, uv), params.p.x);
    textureStore(out_face, id.xy, id.z, vec4<f32>(color, 1.0));
}
