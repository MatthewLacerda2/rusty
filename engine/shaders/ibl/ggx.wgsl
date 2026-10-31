// engine/shaders/ibl/ggx.wgsl — the GGX importance sampling both cube prefilters
// share (#1602): the sky's (`sky/prefilter.wgsl`, #1553) and the reflection probes'
// (`ibl/probe_prefilter.wgsl`). It is `render::ibl::reflection_prefilter`'s on the
// GPU: Hammersley points, half-vectors drawn toward the GGX lobe around `n`, each
// reflected into a light direction under the split-sum N = V = R assumption.

#define_import_path ibl::ggx

#import common::PI

fn radical_inverse_vdc(bits: u32) -> f32 {
    return f32(reverseBits(bits)) * 2.3283064e-10;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

// An orthonormal frame around `n`: columns tangent, bitangent, n.
fn tangent_frame(n: vec3<f32>) -> mat3x3<f32> {
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let tangent = normalize(cross(up, n));
    return mat3x3<f32>(tangent, cross(n, tangent), n);
}

// Sample `i` of `samples` around `frame`'s normal for `a2` (roughness⁴): xyz the
// light direction reflected off the GGX half-vector, w the half-vector's cos θ.
fn ggx_light(i: u32, samples: u32, a2: f32, frame: mat3x3<f32>) -> vec4<f32> {
    let xi = hammersley(i, samples);
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a2 - 1.0) * xi.y));
    let sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
    let h = normalize(frame * vec3<f32>(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta));
    let n = frame[2];
    return vec4<f32>(normalize(2.0 * dot(n, h) * h - n), cos_theta);
}
