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
// is floored at `GRAZING_FLOOR`: a normal map can turn N toward a light that grazed
// Ng, and there an unfloored division blows a whisker of baked light into a
// highlight. On a flat surface (N == Ng) the lobe's own N·L cancels the division
// only above the floor: a light within about 6° of the surface's plane gets
// Ng·L / 0.1 of its live highlight (a lamp 0.5 m over a floor, 8 m off, 62%; a sun
// 2° up, 35%).
//
// Its share, the lamps'. A texel's light also holds bounce, sky and emission, which
// the environment reflection already reflects; only `Baked` lamps' direct light
// reaches the surface as nothing but this lobe. The bake keeps the lamps' part apart
// (`bake::direction`) and stores it in the page's A: `|direct_dir| / luminance(all)`,
// 1 for one point, spot or directional lamp alone, 0 under an open sky or a `Mixed`
// light's bounce, weakened where two lamps meet or where a `Baked` Area light's panel
// fills the view (its arrivals spread, so its vector shortens; it still gives one
// tight highlight toward its centre, not a panel-shaped one). So the lobe's radiance is `texel * share`, and its direction
// the lamps' (the page's RGB, once the share passes a quarter).
//
// Its aim. The page is RGBA8, never block-compressed (#1996): between texels
// bilinear filtering rebuilds a lamp's direction to well under a degree, so a
// polished surface's baked highlight is as round and as sharp as its live one, and
// the lobe takes the surface's own roughness, unwidened. BC7, which the page was,
// put a texel up to 5° off, which a lobe that narrow drew as a sawtooth edge and a
// dot a texel.
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
// the geometric `Ng`, and its dominant direction gives the lobe.
//
// `behind` is a two-sided material's back face (`seen_from_behind`): the bake traced
// only the front's hemisphere, so the page holds what the front saw, while `Ng` and
// the normal map are mirrored through the surface (`facing_normal`, `facing_tangent`).
// The direction is mirrored with them, so the back face shows the front's light
// reshaped as the front reshapes it, and it gets no lobe: no lamp the bake saw is on
// that side.
fn lightmap_light(uv: vec2<f32>, layer: u32, lod: f32, directional: bool, behind: bool, N: vec3<f32>, Ng: vec3<f32>, V: vec3<f32>, F0: vec3<f32>, roughness: f32) -> LightmapLight {
    var out = LightmapLight(textureSampleLevel(t_lightmaps, s_lightmaps, uv, layer, lod).rgb, vec3<f32>(0.0));
    if (directional) {
        var dir = decode_lightmap_direction(textureSampleLevel(t_lightmap_dirs, s_lightmaps, uv, layer, lod));
        if (behind) {
            dir = vec4<f32>(reflect(dir.xyz, Ng), 0.0);
        }
        out.specular = baked_specular(out.irradiance, dir, N, Ng, V, F0, roughness);
        out.irradiance *= lightmap_direction_rebalance(dir.xyz, N, Ng);
    }
    return out;
}

// The least diffuse response the radiance is divided by: Lambert's at a light about
// 84° off the geometric normal (closer to the plane, a flat surface's highlight
// weakens by Ng·L / 0.1, above); the squared wrap's at about 112° (Ng·L ≈ -0.37),
// past the surface's horizon, so under `HalfLambert` it holds only for a light
// behind the surface a normal map has turned toward.
const GRAZING_FLOOR: f32 = 0.1;

// The least lamp share that is a lamp, rescaled from there to 1: a lamp under
// about 6% of a texel's light (the fringe of its range, where a bounce outshines
// it) gives no highlight; the ramp below has faded such a lamp's to a quarter
// already. It once also hid BC7's lending a lamp-free texel share from its block
// (#1695), before direction pages were stored uncompressed (#1996).
const SHARE_FLOOR: f32 = 16.0 / 255.0;

// The lamp share over which the bake turns the page's direction from the mean of
// all the texel's light to the lamps' own (`bake::direction::LAMP_RAMP`, which
// `lightmap::share_tests` holds this line to). Below it the lobe aims partly away
// from the lamps, so it fades with the same ramp: a lamp at its range's edge leaves
// no highlight where no light comes from.
const LAMP_RAMP: f32 = 0.25;

// The least directionality the lobe takes its direction from: below it the page's
// 8-bit vector is too short to aim a highlight.
const MIN_DIRECTIONALITY: f32 = 0.02;

// The specular a directional lightmap texel adds: `texel` its baked E / π (before the
// normal-map rebalance), `dir` its decoded page texel (the direction scaled by its
// directionality, and the lamp share in `w`), `Ng` the geometric normal the bake saw,
// `N` the shading normal, `roughness` already widened by specular AA. Zero in a bake
// capture, where no lamp lights the texel, and where its light has no direction;
// faded by the ramp below `LAMP_RAMP`, where the direction is not all the lamps'.
fn baked_specular(texel: vec3<f32>, dir: vec4<f32>, N: vec3<f32>, Ng: vec3<f32>, V: vec3<f32>, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let aimed = saturate(dir.w / LAMP_RAMP);
    let share = saturate((dir.w - SHARE_FLOOR) / (1.0 - SHARE_FLOOR)) * aimed * lighting.baked_specular;
    let directionality = length(dir.xyz);
    if (share <= 0.0 || directionality < MIN_DIRECTIONALITY) {
        return vec3<f32>(0.0);
    }
    let L = dir.xyz / directionality;
    let radiance = texel * PI * share / max(diffuse_response(dot(Ng, L)), GRAZING_FLOOR);
    return specular_response(N, V, L, radiance, F0, roughness);
}
