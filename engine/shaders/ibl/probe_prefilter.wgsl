// engine/shaders/ibl/probe_prefilter.wgsl — a reflection probe's GGX prefilter on the
// GPU (#1602). One dispatch per rough mip, one invocation per texel of every face:
// the texel's direction is the normal, and the captured cube (mip 0, LDR sRGB) is
// convolved around it for roughness `mip / (mips - 1)` with the sampling the sky cube
// shares (`ibl::ggx`).
//
// It is `render::ibl::reflection_prefilter::cpu::prefilter` texel for texel, which stays
// the reference its tests pin: the same face matrices (the capture's own cameras, so
// the faces' flipped orientation needs no second convention), the same nearest-texel
// lookup of the base faces and the same N·L weighting. The result is sRGB-encoded into
// an RGBA8 face, read back for the KTX2 encode.

#import color::srgb::encode_srgb
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
@group(0) @binding(2) var out_face: texture_storage_2d_array<rgba8unorm, write>;

// World direction through face coords `uv` (clip-space NDC, v up) of `face`.
fn direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let world = params.inv_view_proj[face] * vec4<f32>(uv, 1.0, 1.0);
    return normalize(world.xyz / world.w);
}

// The captured radiance along `dir` (linear: the texture decodes sRGB): the nearest
// texel of the first face whose frustum holds it, as `cube_sample::sample_linear`.
fn capture(dir: vec3<f32>) -> vec3<f32> {
    let res = params.p.z;
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
            return textureLoad(t_capture, vec2<i32>(px, py), i32(face), 0).rgb;
        }
    }
    return vec3<f32>(0.0);
}

// GGX split-sum convolution around `n`, as `reflection_prefilter::cpu::convolve`.
fn convolve(n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let frame = tangent_frame(n);
    let a = roughness * roughness;
    let samples = u32(params.p.w);
    var color = vec3<f32>(0.0);
    var weight = 0.0;
    for (var i = 0u; i < samples; i = i + 1u) {
        let l = ggx_light(i, samples, a * a, frame).xyz;
        let ndotl = dot(n, l);
        if (ndotl > 0.0) {
            color += capture(l) * ndotl;
            weight += ndotl;
        }
    }
    if (weight > 0.0) {
        return color / weight;
    }
    return capture(n);
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
    textureStore(out_face, id.xy, id.z, vec4<f32>(encode_srgb(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0));
}
