// engine/shaders/forward/reflection.wgsl — the forward pass's environment reflection
// (#244, #1015, #1697): up to two reflection probes and the sky (or the scene's
// fallback probe), each probe normalised by the pixel's baked diffuse, and the
// specular occlusion that weighs the whole term. `fs_main` in `shader.wgsl` adds it.

#define_import_path forward::reflection

#import lighting::sh::sh_basis
#import lighting::reflection_probe::{ReflectionProbe, parallax_correct}
#import forward::bindings::{lighting, s_refl_cube, s_skybox, t_refl_cube, t_skybox}

// The environment a surface reflects along the mirror direction `R` (#244, #718). An
// object blends up to two reflection probes (#1584, Unity's Blend Probes, picked per
// object on the CPU): `blend` packs each one's resident index plus one and weight
// (`render::ibl::reflection_probes::pack_blend`), and the sky takes what they leave,
// or the scene's fallback probe when it names one (#1697, Unity's default reflection
// probe). Each probe is normalised by the pixel's baked diffuse (`baked`, #1697); the
// sky is not.
// A probe's direction is parallax-corrected against its box when its box projection
// is on, and its baked, prefiltered cube is sampled (roughness selects the mip within
// its chain, #245), scaled by its intensity and its normalisation. The sky is its cube (#1553), prefiltered
// the same way, so a rough metal reflects a blurred sky: the panorama, or the
// procedural sky's terms tinted with the ambient colour the sky pass draws it with
// (#256). Every sample has an explicit level, so the per-instance branches are safe.
// A light-probe capture turns it off (`lighting.env_reflection`, #1668), a uniform
// branch: Unity's light-probe bake carries no specular.
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
        // The scene's fallback probe (#1697) takes what the sky took: it is always
        // resident, in slot 0, so this branch is uniform.
        if (lighting.refl_probes[0].params.z > 0.5) {
            out += probe_reflection(lighting.refl_probes[0], world_pos, R, b, roughness, baked) * rest;
        } else {
            out += sky_reflection(R, roughness) * rest;
        }
    }
    return out;
}

// One resident probe's reflection along `R`, normalised by the baked diffuse at the
// pixel (`probe_normalisation`; `b` is the SH basis along the normal).
fn probe_reflection(p: ReflectionProbe, world_pos: vec3<f32>, R: vec3<f32>, b: array<f32, 9>, roughness: f32, baked: vec2<f32>) -> vec3<f32> {
    let dir = select(R, parallax_correct(p, world_pos, R), p.params.y > 0.5);
    let lod = p.mip_base + roughness * (p.mip_count - 1.0);
    let cube = textureSampleLevel(t_refl_cube, s_refl_cube, dir, p.layer, lod).rgb;
    return cube * p.params.x * probe_normalisation(p, b, baked);
}

// Probe normalisation (#1697; UE's lightmap mixing, Frostbite's and Call of Duty's
// probe normalisation): a probe's cube shows its room under the light at its capture
// point, so a pixel in a corner the lightmap holds dark would reflect the lit room at
// full strength. Its reflection is scaled by the pixel's baked diffuse over the
// irradiance the probe stored (`b` is `sh_basis(N)`), both luminance as E / pi along
// `N`, held to [0, 1]: it removes leaks and never brightens, so a floor lit brighter
// than the bounce light the probe saw keeps its reflection at 1x. `baked` is
// `baked_diffuse`'s: a pixel with no lightmap or light probe, or a probe baked without
// the irradiance (`params.w`, also 0 in every bake capture), scales by 1.
// `stored` is floored at STORED_FLOOR of the probe's mean (its constant term): an L2
// SH of one dominant lamp rings below zero for normals turned 104-145 degrees from
// it, and that says nothing about what the cube shows along R. Both sides add
// NORMALISE_BIAS, so where neither holds any light (a sky probe under zero ambient
// over an unlit floor) the scale is 1, not 0 / 0. `uniform::normalisation` mirrors
// this on the CPU for the tests, constants included.
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

// What `probe_normalisation` compares with: the luminance of the pixel's baked
// diffuse `ambient` (E / pi: its lightmap, or its light-probe SH), and 1 when it has
// one (`baked`), 0 when it shades with the flat ambient. Rec. 709, as
// `core::color::luminance`.
fn baked_diffuse(ambient: vec3<f32>, baked: bool) -> vec2<f32> {
    return vec2<f32>(dot(ambient, vec3<f32>(0.2126, 0.7152, 0.0722)), select(0.0, 1.0, baked));
}

// Specular occlusion from ambient occlusion (#1697): Lagarde & de Rousiers, *Moving
// Frostbite to PBR*, as HDRP's `GetSpecularOcclusionFromAmbientOcclusion`
// (CommonLighting.hlsl), which HDRP calls with `PerceptualRoughnessToRoughness`, so
// `roughness` (perceptual, as everywhere here) is squared first. A rough lobe is
// occluded about as much as the AO says, a glossy one less, and less still head-on;
// with `ao` 1 it is 1.
fn specular_occlusion(n_dot_v: f32, ao: f32, roughness: f32) -> f32 {
    let alpha = roughness * roughness;
    return saturate(pow(max(n_dot_v + ao, 1e-6), exp2(-16.0 * alpha - 1.0)) - 1.0 + ao);
}

// The sky's reflection along `R`.
fn sky_reflection(R: vec3<f32>, roughness: f32) -> vec3<f32> {
    let lod = roughness * f32(textureNumLevels(t_skybox) - 1u);
    let sky = textureSampleLevel(t_skybox, s_skybox, R, lod);
    if (lighting.sky_textured > 0.5) {
        return sky.rgb;
    }
    return sky.rgb + lighting.ambient.color * sky.a;
}
