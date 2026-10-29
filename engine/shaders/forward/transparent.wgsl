// engine/shaders/forward/transparent.wgsl — how a surface's lit colour leaves the
// forward shader (#1523): the scene fog, and for a transparent surface the alpha its
// blend mode reads. `surface_color` returns through `composite`; an opaque surface
// leaves it exactly as `apply_fog` would, at alpha 1.

#define_import_path forward::transparent

#import forward::bindings::{camera, entity}
#import common::fog_factor

// `lit` fogged, with the alpha the pipeline blends with. Preserve specular (URP's
// `_ALPHAPREMULTIPLY_ON`) already multiplied the diffuse by `alpha` and blends
// premultiplied, so the fog is premultiplied too, and the opacity rises with the
// reflection: URP's `alpha * oneMinusReflectivity + reflectivity`, with the
// reflectivity the environment BRDF's weight at this view angle, so a grazing
// surface reflects more of the scene and shows less of what is behind it.
fn composite(lit: vec3<f32>, alpha: f32, env_brdf: vec3<f32>, world: vec3<f32>) -> vec4<f32> {
    let fog = fog_factor(camera.fog, world, camera.camera_pos);
    if (entity.premultiply_diffuse == 0u) {
        return vec4<f32>(mix(lit, camera.fog.color, fog), alpha);
    }
    let reflected = max(env_brdf.r, max(env_brdf.g, env_brdf.b));
    let opacity = alpha + (1.0 - alpha) * saturate(reflected);
    return vec4<f32>(mix(lit, camera.fog.color * opacity, fog), opacity);
}
