// engine/shaders/sky/prefilter.wgsl — the sky's reflection cube (#1553): the sky
// captured into a cube and GGX-prefiltered into a roughness mip chain, Unity's
// skybox-as-default-reflection-probe. One dispatch per mip, one invocation per texel
// of every face; mip 0 is the sky itself (roughness 0), each later mip the sky
// convolved for roughness `mip / (mips - 1)`, read at
// `roughness * (mips - 1)` by `forward::lighting::environment_reflection` exactly as a
// probe cube is.
//
// The convolution is `render::ibl::reflection_prefilter`'s, on the GPU: the split-sum
// N = V = R assumption, Hammersley half-vectors drawn toward the GGX lobe, each
// reflected into a light direction and weighted by N·L. Two sources:
// - the panorama (`params.p.z == 1`): sampled at the mip whose texel matches the
//   sample's solid angle (filtered importance sampling), so a few taps blur
//   without sparkle;
// - the procedural sky: `common::procedural_sky` is affine in its tint, so the cube
//   stores its two terms (rgb: the sky at tint 0, a: what tint 1 adds) and the
//   forward pass applies the ambient colour when it samples. The tint (a scripted day
//   cycle) never rebakes it.

#import common::{PI, equirect_uv, procedural_sky}

struct Params {
    // x: roughness, y: face size in texels, z: 1 for the panorama (0: procedural),
    // w: the importance sample count, `render::ibl::reflection_prefilter`'s
    // `SAMPLE_COUNT`, so a probe and the sky blur rough metal alike.
    p: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var t_panorama: texture_2d<f32>;
@group(0) @binding(2) var s_panorama: sampler;
@group(0) @binding(3) var out_face: texture_storage_2d_array<rgba16float, write>;

// The world direction through texel centre `uv` (0..1, v down) of cube face `face`
// (+X, -X, +Y, -Y, +Z, -Z): the cube-map convention a `texture_cube` sample follows.
fn face_direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let s = uv.x * 2.0 - 1.0;
    let t = uv.y * 2.0 - 1.0;
    switch face {
        case 0u: { return normalize(vec3<f32>(1.0, -t, -s)); }
        case 1u: { return normalize(vec3<f32>(-1.0, -t, s)); }
        case 2u: { return normalize(vec3<f32>(s, 1.0, t)); }
        case 3u: { return normalize(vec3<f32>(s, -1.0, -t)); }
        case 4u: { return normalize(vec3<f32>(s, -t, 1.0)); }
        default: { return normalize(vec3<f32>(-s, -t, -1.0)); }
    }
}

// The procedural sky's two terms along `dir`: rgb at tint 0, a the tint's weight.
fn procedural_terms(dir: vec3<f32>) -> vec4<f32> {
    let base = procedural_sky(vec3<f32>(0.0), dir);
    let full = procedural_sky(vec3<f32>(1.0), dir);
    return vec4<f32>(base, full.x - base.x);
}

// The source along `dir`, read at panorama mip `lod`.
fn source(dir: vec3<f32>, lod: f32) -> vec4<f32> {
    if (params.p.z > 0.5) {
        return vec4<f32>(textureSampleLevel(t_panorama, s_panorama, equirect_uv(dir), lod).rgb, 1.0);
    }
    return procedural_terms(dir);
}

// The panorama's average texel solid angle at its base mip.
fn panorama_texel() -> f32 {
    let dims = vec2<f32>(textureDimensions(t_panorama, 0));
    return 4.0 * PI / (dims.x * dims.y);
}

// The panorama mip whose texel covers what one texel of a `size`² cube face does
// (≈ 4π / (6·size²)), so the mirror mip reads the sky prefiltered to its own
// resolution and thin bright detail doesn't alias into the face.
fn mirror_lod(size: f32) -> f32 {
    let cube_texel = 4.0 * PI / (6.0 * size * size);
    let last = f32(textureNumLevels(t_panorama) - 1u);
    return clamp(0.5 * log2(cube_texel / panorama_texel()), 0.0, last);
}

fn radical_inverse_vdc(bits: u32) -> f32 {
    return f32(reverseBits(bits)) * 2.3283064e-10;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

// GGX split-sum convolution around `n`, as `reflection_prefilter::convolve`.
fn convolve(n: vec3<f32>, roughness: f32) -> vec4<f32> {
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let tangent = normalize(cross(up, n));
    let bitangent = cross(n, tangent);
    let a = roughness * roughness;
    let a2 = a * a;
    let texel = panorama_texel();
    let last = f32(textureNumLevels(t_panorama) - 1u);
    let samples = u32(params.p.w);
    var color = vec4<f32>(0.0);
    var weight = 0.0;
    for (var i = 0u; i < samples; i = i + 1u) {
        let xi = hammersley(i, samples);
        let phi = 2.0 * PI * xi.x;
        let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a2 - 1.0) * xi.y));
        let sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
        let h = normalize(tangent * sin_theta * cos(phi) + bitangent * sin_theta * sin(phi) + n * cos_theta);
        let l = normalize(2.0 * dot(n, h) * h - n);
        let ndotl = dot(n, l);
        if (ndotl > 0.0) {
            // GGX D at this half-vector; with N = V its pdf over L is D / 4.
            let d = (a2 / (PI * pow(cos_theta * cos_theta * (a2 - 1.0) + 1.0, 2.0)));
            let sample_angle = 4.0 / (f32(samples) * max(d, 1e-4));
            let lod = clamp(0.5 * log2(sample_angle / texel) + 1.0, 0.0, last);
            color += source(l, lod) * ndotl;
            weight += ndotl;
        }
    }
    if (weight > 0.0) {
        return color / weight;
    }
    return source(n, 0.0);
}

@compute @workgroup_size(8, 8, 1)
fn cs_prefilter(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = u32(params.p.y);
    if (id.x >= size || id.y >= size) {
        return;
    }
    let uv = (vec2<f32>(id.xy) + 0.5) / f32(size);
    let n = face_direction(id.z, uv);
    let roughness = params.p.x;
    var out: vec4<f32>;
    if (roughness <= 0.0) {
        out = source(n, mirror_lod(f32(size)));
    } else {
        out = convolve(n, roughness);
    }
    textureStore(out_face, id.xy, id.z, out);
}
