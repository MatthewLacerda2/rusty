
#define_import_path forward::reflection

#import lighting::sh::sh_basis
#import lighting::reflection_probe::{ReflectionProbe, parallax_correct}
#import forward::bindings::{lighting, s_linear_clamp, t_refl_cube, t_skybox}

// Unity URP's analytic environment BRDF (`BRDF.hlsl` `EnvironmentBRDFSpecular`). Its
// `surfaceReduction` is the energy a rough lobe loses, so a rough metal reads blurry, not
// black. The angle is `|N.V|`: a normal map can lean away from the camera on a face turned
// toward it, which would take the grazing Fresnel. `fs_main` passes the authored roughness, not
// the widened one: `specular_aa`'s spread would take `grazing` from 0.96 to 0.51.
fn environment_brdf(F0: vec3<f32>, F90: f32, dielectric_reflectivity: f32, metallic: f32, roughness: f32, N: vec3<f32>, V: vec3<f32>) -> vec3<f32> {
    let reflectivity = dielectric_reflectivity + (1.0 - dielectric_reflectivity) * metallic;
    let grazing = saturate(1.0 - roughness + reflectivity) * F90;
    let alpha = roughness * roughness;
    let fresnel = pow(1.0 - saturate(abs(dot(N, V))), 4.0);
    return mix(F0, vec3<f32>(grazing), fresnel) / (alpha * alpha + 1.0);
}

// Unity's Blend Probes: `blend` packs each probe's resident index plus one and weight
// (`render::ibl::reflection_probes::pack_blend`). Every sample has an explicit level,
// so the per-instance branches are safe. A light-probe capture turns it off: Unity's
// light-probe bake carries no specular.
fn environment_reflection(world_pos: vec3<f32>, raw_r: vec3<f32>, N: vec3<f32>, roughness: f32, blend: u32, baked: vec2<f32>) -> vec3<f32> {
    if (lighting.env_reflection < 0.5) {
        return vec3<f32>(0.0);
    }
    let R = normalize(raw_r);
    let b = sh_basis(N);
    let first = blend & 63u;
    let second = (blend >> 6u) & 63u;
    let first_weight = f32((blend >> 12u) & 1023u) / 1023.0;
    let second_weight = f32((blend >> 22u) & 1023u) / 1023.0;
    var out = vec3<f32>(0.0);
    if (first > 0u) {
        out += probe_reflection(lighting.refl_probes[first - 1u], world_pos, R, b, roughness, baked) * first_weight;
    }
    if (second > 0u) {
        out += probe_reflection(lighting.refl_probes[second - 1u], world_pos, R, b, roughness, baked) * second_weight;
    }
    let rest = select(1.0, max(1.0 - first_weight - second_weight, 0.0), first > 0u);
    if (rest > 0.002) {
        if (lighting.refl_probes[0].params.z > 0.5) {
            out += probe_reflection(lighting.refl_probes[0], world_pos, R, b, roughness, baked) * rest;
        } else {
            out += sky_reflection(R, roughness) * rest;
        }
    }
    return out;
}

fn probe_reflection(p: ReflectionProbe, world_pos: vec3<f32>, R: vec3<f32>, b: array<f32, 9>, roughness: f32, baked: vec2<f32>) -> vec3<f32> {
    let dir = select(R, parallax_correct(p, world_pos, R), p.params.y > 0.5);
    let lod = p.mip_base + roughness * (p.mip_count - 1.0);
    let cube = textureSampleLevel(t_refl_cube, s_linear_clamp, dir, p.layer, lod).rgb;
    return cube * p.params.x * probe_normalisation(p, b, baked);
}

// Probe normalisation (`docs/api/Reflection.md`): baked diffuse over the probe's stored
// irradiance along N, both E / π luminance, held to [0, 1].
// `ibl::reflection_probes::uniform::normalisation` mirrors it on the CPU for the tests,
// constants included: change both together.
// `stored` never drops below `STORED_FLOOR` of the probe's mean, where an L2 SH of one bright
// lamp rings below zero; with `NORMALISE_BIAS` (E / π) on both sides, two zeros scale by 1.
const STORED_FLOOR: f32 = 0.25;
const NORMALISE_BIAS: f32 = 0.001;
fn probe_normalisation(p: ReflectionProbe, b: array<f32, 9>, baked: vec2<f32>) -> f32 {
    if (p.params.w < 0.5 || baked.y < 0.5) {
        return 1.0;
    }
    let stored = dot(p.irradiance[0], vec4<f32>(b[0], b[1], b[2], b[3]))
        + dot(p.irradiance[1], vec4<f32>(b[4], b[5], b[6], b[7]))
        + p.irradiance[2].x * b[8];
    let floor = p.irradiance[0].x * b[0] * STORED_FLOOR;
    return saturate((baked.x + NORMALISE_BIAS) / (max(stored, floor) + NORMALISE_BIAS));
}

fn baked_diffuse(ambient: vec3<f32>, baked: bool) -> vec2<f32> {
    return vec2<f32>(dot(ambient, vec3<f32>(0.2126, 0.7152, 0.0722)), select(0.0, 1.0, baked));
}

// Lagarde & de Rousiers, *Moving Frostbite to PBR*, as HDRP's
// `GetSpecularOcclusionFromAmbientOcclusion`, which squares perceptual roughness first.
fn specular_occlusion(n_dot_v: f32, ao: f32, roughness: f32) -> f32 {
    let alpha = roughness * roughness;
    return saturate(pow(max(n_dot_v + ao, 1e-6), exp2(-16.0 * alpha - 1.0)) - 1.0 + ao);
}

fn sky_reflection(R: vec3<f32>, roughness: f32) -> vec3<f32> {
    let lod = roughness * f32(textureNumLevels(t_skybox) - 1u);
    let sky = textureSampleLevel(t_skybox, s_linear_clamp, R, lod);
    if (lighting.sky_textured > 0.5) {
        return sky.rgb;
    }
    return sky.rgb + lighting.ambient.color * sky.a;
}
