// engine/shaders/forward/transparent.wgsl — the forward shader's transparent-surface
// terms (#1523): how a surface's lit colour leaves it (the scene fog, and for a
// transparent surface the alpha its blend mode reads and, when it refracts or
// absorbs, what is behind it), and thin translucency's back-lit light. Every colour
// entry point returns through `composite`; an opaque surface leaves it exactly as
// `apply_fog` would, at alpha 1.

#define_import_path forward::transparent

#import forward::bindings::{camera, entity, s_opaque, t_opaque}
#import common::{PI, fog_factor}

// `lit` fogged, with the alpha the pipeline blends with. Preserve specular (URP's
// `_ALPHAPREMULTIPLY_ON`) already multiplied the diffuse by `alpha` and blends
// premultiplied, so the fog is premultiplied too, and the opacity rises with the
// reflection: URP's `alpha * oneMinusReflectivity + reflectivity`, with the
// reflectivity the environment BRDF's weight at this view angle, so a grazing
// surface reflects more of the scene and shows less of what is behind it. A surface
// over the opaque copy (`uses_opaque`) lays what is behind it under itself, bent and
// absorbed, and writes alpha 1: the premultiplied blend then keeps it whole.
// `world`, `N` and `pixel` are the fragment's world position, shading normal and
// framebuffer position.
fn composite(lit: vec3<f32>, alpha: f32, env_brdf: vec3<f32>, world: vec3<f32>, N: vec3<f32>, pixel: vec2<f32>) -> vec4<f32> {
    let fog = fog_factor(camera.fog, world, camera.camera_pos);
    if (entity.premultiply_diffuse == 0u && entity.uses_opaque == 0u) {
        return vec4<f32>(mix(lit, camera.fog.color, fog), alpha);
    }
    var opacity = alpha;
    var color = lit;
    if (entity.premultiply_diffuse == 1u) {
        let reflected = max(env_brdf.r, max(env_brdf.g, env_brdf.b));
        opacity = alpha + (1.0 - alpha) * saturate(reflected);
    } else if (entity.blend_mode == 0u) {
        // Straight alpha over the copy: premultiply here, the blend no longer does.
        color = lit * alpha;
    }
    color = mix(color, camera.fog.color * opacity, fog);
    if (entity.uses_opaque == 0u) {
        return vec4<f32>(color, opacity);
    }
    return vec4<f32>(color + (1.0 - opacity) * behind(world, N, pixel), 1.0);
}

// The diffuse light a thin translucent surface lets through from a light behind it
// (#1523): HDRP's thin *Translucent* type, a back-face Lambert term scaled by the
// transmission colour times its amount. `N` faces the viewer, so a light behind the
// surface has `dot(N, L) < 0`; `radiance` is the light shadowed at the back face.
fn transmitted(N: vec3<f32>, L: vec3<f32>, radiance: vec3<f32>, albedo: vec3<f32>) -> vec3<f32> {
    return entity.transmission.rgb * albedo / PI * radiance * saturate(dot(-N, L));
}

// Where the opaque copy holds no surface (the sky): `opaque_copy.wgsl`'s
// `SKY_DISTANCE`, the largest finite `f16`, read back with room to spare.
const SKY_DISTANCE: f32 = 60000.0;

// What a refracting or absorbing surface at `world` (shading normal `N`, at
// framebuffer `pixel`) shows of the opaque scene behind it.
//
// Refraction is HDRP's (`Refraction.hlsl`, `RefractionModelBox`, which its UI calls
// *Planar*; *Thin* is the same slab 5 mm thick): the view ray refracts into a slab
// `thickness` deep along the normal, normal map included, and leaves it parallel to
// the view ray, so the opaque copy is read where the exit point projects. As in
// HDRP's `EvaluateBSDF_ScreenspaceRefraction`, an exit point that lands on something
// in front of the surface reads straight through instead. Absorption is HDRP's
// Beer-Lambert `exp(-absorption * distance)`, over the distance from the surface to
// the opaque surface behind it along the view (the copy's eye distance less the
// surface's), so shallow water stays clear and deep water takes the transmittance
// colour; seen against the sky it absorbs over the material's `thickness`
// (`absorption.w`), whatever its refraction model.
fn behind(world: vec3<f32>, N: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(t_opaque));
    let eye_to_surface = distance(camera.camera_pos, world);
    var uv = pixel / size;
    if (entity.refraction_model != 0u) {
        let V = normalize(camera.camera_pos - world);
        // Both faces are drawn: bend through the face toward the viewer.
        let Nf = faceForward(N, -V, N);
        let R = refract(-V, Nf, 1.0 / entity.ior);
        let dist = entity.thickness / max(dot(R, -Nf), 1e-5);
        let clip = camera.view_proj * vec4<f32>(world + R * dist, 1.0);
        if (clip.w > 1e-4) {
            let hit = vec2<f32>(clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5);
            if (textureSampleLevel(t_opaque, s_opaque, hit, 0.0).a > eye_to_surface) {
                uv = hit;
            }
        }
    }
    let scene = textureSampleLevel(t_opaque, s_opaque, uv, 0.0);
    var depth = max(scene.a - eye_to_surface, 0.0);
    if (scene.a >= SKY_DISTANCE) {
        depth = entity.absorption.w;
    }
    return scene.rgb * exp(-entity.absorption.rgb * depth);
}
