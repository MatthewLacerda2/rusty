
#define_import_path forward::lightmap

#import common::PI
#import forward::bindings::{lighting, s_linear_clamp, t_lightmap_dirs, t_lightmaps}
#import forward::lighting::{decode_lightmap_direction, diffuse_response, lightmap_direction_rebalance, specular_response}

// A lightmapped surface's baked light: `irradiance` its E / pi, the Lambert response `* albedo`
// gives as a realtime light's does, and `specular` the directional page's lobe (zero without
// one), which the caller weighs by its specular occlusion.
struct LightmapLight {
    irradiance: vec3<f32>,
    specular: vec3<f32>,
};

fn lightmap_light(uv: vec2<f32>, layer: u32, lod: f32, directional: bool, behind: bool, N: vec3<f32>, Ng: vec3<f32>, V: vec3<f32>, F0: vec3<f32>, F90: f32, roughness: f32) -> LightmapLight {
    var out = LightmapLight(textureSampleLevel(t_lightmaps, s_linear_clamp, uv, layer, lod).rgb, vec3<f32>(0.0));
    if (directional) {
        var dir = decode_lightmap_direction(textureSampleLevel(t_lightmap_dirs, s_linear_clamp, uv, layer, lod));
        if (behind) {
            dir = vec4<f32>(reflect(dir.xyz, Ng), 0.0);
        }
        out.specular = baked_specular(out.irradiance, dir, N, Ng, V, F0, F90, roughness);
        out.irradiance *= lightmap_direction_rebalance(dir.xyz, N, Ng);
    }
    return out;
}

// The least diffuse response the radiance is divided by: Lambert's at a light about 84° off the
// geometric normal (closer to the plane, a flat surface's highlight weakens by Ng·L / 0.1,
// above); the squared wrap's at about 112° (Ng·L ≈ -0.37), past the surface's horizon, so under
// `HalfLambert` it holds only for a light behind the surface a normal map has turned toward.
const GRAZING_FLOOR: f32 = 0.1;

const LAMP_RAMP: f32 = 0.25;

// The least directionality the lobe takes its direction from: below it the page's 8-bit vector
// is too short to aim a highlight.
const MIN_DIRECTIONALITY: f32 = 0.02;

fn baked_specular(texel: vec3<f32>, dir: vec4<f32>, N: vec3<f32>, Ng: vec3<f32>, V: vec3<f32>, F0: vec3<f32>, F90: f32, roughness: f32) -> vec3<f32> {
    let aimed = saturate(dir.w / LAMP_RAMP);
    let share = dir.w * aimed * lighting.baked_specular;
    let directionality = length(dir.xyz);
    if (share <= 0.0 || directionality < MIN_DIRECTIONALITY) {
        return vec3<f32>(0.0);
    }
    let L = dir.xyz / directionality;
    let radiance = texel * PI * share / max(diffuse_response(dot(Ng, L)), GRAZING_FLOOR);
    return specular_response(N, V, L, radiance, F0, F90, roughness);
}
