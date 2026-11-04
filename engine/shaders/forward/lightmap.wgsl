// engine/shaders/forward/lightmap.wgsl — what a lightmapped surface reads from its
// lightmap (#438): the baked irradiance, reshaped by a directional page (#810), and
// that page's one specular lobe.
//
// The lobe is directional specular (#1695, step 1): `Baked` lamps reach a lightmapped surface only through
// its lightmap, which holds their diffuse light, so a metal rail or a polished floor
// beside one showed no highlight. A directional page (#810) also records where each
// texel's light came from, so this treats that dominant direction as one light and
// evaluates the GGX lobe once, as Bakery's *Directional Specular* and Frostbite do.
// Unity has no such term.
//
// Its light's strength. The texel holds E / π at the geometric normal `Ng`, E being
// the irradiance a live light's `radiance` gives through the scene's diffuse response
// (`diffuse_response`, Lambert's N·L or the squared wrap). So one light from `L`
// that would bake this texel has radiance `texel * π / response(Ng·L)`. The response
// is floored at `GRAZING_FLOOR`: on a flat surface (N == Ng) the lobe's own N·L
// cancels it, but a normal map can turn N toward a light that grazed Ng, and there
// an unfloored division blows a whisker of baked light into a highlight.
//
// Its weight, the directionality. A page's direction is the luminance-weighted mean
// of where light arrived from, so one lamp gives 1 and light from every way above
// the surface (an even sky) gives the cosine-weighted mean, 2/3, not 0
// (`an_open_floor_under_the_sky_points_up_but_not_fully`). That evenly spread light is
// the environment reflection's to reflect, so the lobe counts only the directionality
// above 2/3 (`lobe_weight`): one lamp keeps its full highlight, a sky-lit floor gets
// none, and two lamps lighting one texel give one highlight between them, weakened.
//
// No AO: the lobe stands in for direct light, which AO doesn't touch here, and the
// static occluders' shadows are already in the bake. It is off in every bake capture
// (`lighting.baked_specular`), so no view-dependent highlight is frozen into a probe.
// It is on for every lit, directionally lightmapped surface; a per-material switch
// would be a `MaterialAsset` bool read here, as `forward::specular_aa` notes for its own.

#define_import_path forward::lightmap

#import common::PI
#import forward::bindings::{lighting, s_lightmaps, t_lightmap_dirs, t_lightmaps}
#import forward::lighting::{decode_lightmap_direction, diffuse_response, lightmap_direction_rebalance, specular_response}

// A lightmapped surface's baked light: `irradiance` its E / pi, the Lambert response
// `* albedo` gives as a realtime light's does, and `specular` the directional page's
// lobe (zero without one), which AO does not weigh.
struct LightmapLight {
    irradiance: vec3<f32>,
    specular: vec3<f32>,
};

// Page `layer`'s light at `uv`, sampled at mip `lod` (`lightmap_level`, taken before
// any branch: this read may diverge). The colour page holds bounce, sky, emission and
// `Baked` lights' direct light, stored linear (BC6H, #1694) as E / pi. With a
// `directional` page, the normal-mapped (and decal-perturbed) `N` reshapes it against
// the geometric `Ng` the bake saw, and its dominant direction gives the lobe.
fn lightmap_light(uv: vec2<f32>, layer: u32, lod: f32, directional: bool, N: vec3<f32>, Ng: vec3<f32>, V: vec3<f32>, F0: vec3<f32>, roughness: f32) -> LightmapLight {
    var out = LightmapLight(textureSampleLevel(t_lightmaps, s_lightmaps, uv, layer, lod).rgb, vec3<f32>(0.0));
    if (directional) {
        let dir = decode_lightmap_direction(textureSampleLevel(t_lightmap_dirs, s_lightmaps, uv, layer, lod));
        out.specular = baked_specular(out.irradiance, dir, N, Ng, V, F0, roughness);
        out.irradiance *= lightmap_direction_rebalance(dir, N, Ng);
    }
    return out;
}

// The least diffuse response the radiance is divided by: a light at about 84° off
// the geometric normal (Lambert) or straight along the surface (the wrap's 1/4).
const GRAZING_FLOOR: f32 = 0.1;

// The directionality of light arriving evenly over the hemisphere a surface faces.
const HEMISPHERE_DIRECTIONALITY: f32 = 2.0 / 3.0;

// How much of a texel's light the lobe treats as coming from its dominant direction:
// 0 at or below an even hemisphere's directionality, 1 for one lamp.
fn lobe_weight(directionality: f32) -> f32 {
    return saturate((directionality - HEMISPHERE_DIRECTIONALITY) / (1.0 - HEMISPHERE_DIRECTIONALITY));
}

// The specular a directional lightmap texel adds: `texel` its baked E / π (before the
// normal-map rebalance), `dir` its decoded direction scaled by its directionality,
// `Ng` the geometric normal the bake saw, `N` the shading normal, `roughness` already
// widened by specular AA. Zero in a bake capture and where the light has no direction.
fn baked_specular(texel: vec3<f32>, dir: vec3<f32>, N: vec3<f32>, Ng: vec3<f32>, V: vec3<f32>, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let directionality = length(dir);
    let weight = lobe_weight(directionality) * lighting.baked_specular;
    if (weight <= 0.0) {
        return vec3<f32>(0.0);
    }
    let L = dir / directionality;
    let radiance = texel * PI / max(diffuse_response(dot(Ng, L)), GRAZING_FLOOR) * weight;
    return specular_response(N, V, L, radiance, F0, roughness);
}
