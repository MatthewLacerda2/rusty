// engine/shaders/forward/transparent.wgsl — the forward shader's transparent-surface
// terms (#1523): how a surface's lit colour leaves it (the scene fog and the
// volumetric fog, #1703, and for a
// transparent surface the alpha its blend mode reads and, when it refracts or
// absorbs, what is behind it), and thin translucency's back-lit light. Every colour
// entry point returns through `composite`; an opaque surface leaves it exactly as
// `fogged` would, at alpha 1. A transparent surface may also trace screen-space
// reflections (`reflect_ssr`, #1570).

#define_import_path forward::transparent

#import forward::bindings::{camera, material, s_fog_volume, s_opaque, t_fog_volume, t_opaque}
#import common::{PI, fog_factor}
#import fog::volumetric::volumetric_fog
#import lighting::ssr_march
#import lighting::ssr_surface
#import reactive

// The volumetric fog between the eye and `world` (#1703): rgb the light it adds,
// a what gets through. No fetch where the camera reads no volume.
fn volumetric(world: vec3<f32>) -> vec4<f32> {
    return volumetric_fog(t_fog_volume, s_fog_volume, camera.volumetric, camera.view_proj, camera.clusters.view_z, world);
}

// `lit` behind both fogs: the distance and height fog, then the volumetric fog
// as the material's blend takes it (`blended_air`; an opaque surface's is Alpha).
// Unlit surfaces return through it.
fn fogged(lit: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let fog = fog_factor(camera.fog, world, camera.camera_pos);
    return blended_air(mix(lit, camera.fog.color, fog), volumetric(world), 1.0);
}

// How much of a surface's own light the fogs take, as one factor: the distance
// and height fog's, and with the volumetric fog `1 - (1 - fog) * transmittance`.
// What SSR's record and SSIL's realtime share are dimmed by, so neither adds light
// the fog took (#1703). Exactly `fog_factor` where the camera reads no volume.
fn surface_fog(world: vec3<f32>) -> f32 {
    let fog = fog_factor(camera.fog, world, camera.camera_pos);
    if (camera.volumetric.x < 0.5) {
        return fog;
    }
    return 1.0 - (1.0 - fog) * volumetric(world).a;
}

// `color` behind the volumetric fog's `air`, for the blend the material draws with.
// An Alpha or Premultiply surface replaces what is behind it by `coverage`, so it
// takes the air's light in front of it, as much as it covers. An Additive surface
// adds onto what is behind it, which already carries that air: it is only dimmed,
// as additive particles are. A Multiply surface scales what is behind it: seen
// through the air its tint fades toward white, so a white sheet stays invisible.
fn blended_air(color: vec3<f32>, air: vec4<f32>, coverage: f32) -> vec3<f32> {
    let mode = material().blend_mode;
    if (mode == 2u) {
        return color * air.a;
    }
    if (mode == 3u) {
        return mix(vec3<f32>(1.0), color, air.a);
    }
    return color * air.a + air.rgb * coverage;
}

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
    let m = material();
    let fog = fog_factor(camera.fog, world, camera.camera_pos);
    let air = volumetric(world);
    if (m.premultiply_diffuse == 0u && m.uses_opaque == 0u) {
        return vec4<f32>(blended_air(mix(lit, camera.fog.color, fog), air, 1.0), alpha);
    }
    var opacity = alpha;
    var color = lit;
    if (m.premultiply_diffuse == 1u) {
        let reflected = max(env_brdf.r, max(env_brdf.g, env_brdf.b));
        opacity = alpha + (1.0 - alpha) * saturate(reflected);
    } else if (m.blend_mode == 0u) {
        // Straight alpha over the copy: premultiply here, the blend no longer does.
        color = lit * alpha;
    }
    color = mix(color, camera.fog.color * opacity, fog);
    // Premultiplied: the air's light in front of the surface, as much of it as the
    // surface covers (what is behind it already carries its own, further fog).
    color = blended_air(color, air, opacity);
    if (m.uses_opaque == 0u) {
        return vec4<f32>(color, opacity);
    }
    return vec4<f32>(color + (1.0 - opacity) * behind(world, N, pixel), 1.0);
}

// The brightest a forward colour channel is written (#1699): below `Rgba16Float`'s
// 65504, with room for the rounding to f16.
const MAX_HDR: f32 = 60000.0;

// `c` with its colour channels within +-`MAX_HDR`, alpha as is. Every colour entry
// point returns through it, so a pixel that would overflow half-float (polished
// metal a hand's width from a lamp: inverse-square light has no ceiling) stores a
// large finite value, not `Inf`, which would turn bloom's bright pass NaN and its
// blur would spread into a black block; SSR and the exposure meter read it too.
fn finite_hdr(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(clamp(c.rgb, vec3<f32>(-MAX_HDR), vec3<f32>(MAX_HDR)), c.a);
}

// HDRP's *Receive SSR Transparent* (#1570): the environment reflection `reflection`
// (step 5's probe or sky term, added under `weight`) with what the reflected ray `R`
// from `world` hits on screen in its place, as the SSR pass does for an opaque pixel
// (`reflection + (weight × hit − reflection) × fade`), faded by the same smoothness
// and screen-edge rules. The ray marches the opaque copy, the camera's opaque image
// after SSR with each pixel's eye distance, from the surface's own depth and normal,
// so it needs no prepass; a miss keeps the probe. Only a material with `receive_ssr`
// on a camera that marches (`camera.ssr.x`, the base camera of a view running SSR)
// traces: both are uniform, so every other surface returns at once. `N` is the
// shading normal, `roughness` the authored perceptual roughness (not specular AA's
// widened one, #1702: a far busy pane keeps its SSR). The copy has no mip pyramid,
// so the hit is sharp: the camera's *Minimum Smoothness* here is never below the
// sharp floor (`render::postfx::effects::ssr::transparent::SHARP_SMOOTHNESS`).
fn reflect_ssr(reflection: vec3<f32>, weight: vec3<f32>, world: vec3<f32>, N: vec3<f32>, R: vec3<f32>, roughness: f32) -> vec3<f32> {
    // A pixel too rough to reflect (HDRP's *Minimum Smoothness*) marches nothing,
    // as the SSR pass discards a surface its fade refuses.
    let fade = ssr_surface::smoothness_fade(roughness, camera.ssr.z, camera.ssr.w);
    if (material().receive_ssr == 0u || camera.ssr.x < 1.0 || fade <= 0.0) {
        return reflection;
    }
    let f = ssr_march::Frame(camera.view_proj, camera.camera_pos, vec3<f32>(0.0), vec4<f32>(0.0, 0.0, 0.0, 1.0));
    // Off the side the ray leaves from: both faces are drawn.
    let origin = world + faceForward(N, -R, N) * 0.02;
    let hit = ssr_march::trace(f, t_opaque, t_opaque, s_opaque, origin, R, i32(camera.ssr.x), camera.ssr.y, 0.0);
    let k = hit.w * fade;
    return reflection + (weight * hit.rgb - reflection) * k;
}

// The diffuse light a thin translucent surface lets through from a light behind it
// (#1523): HDRP's thin *Translucent* type, a back-face Lambert term scaled by the
// transmission colour times its amount. `N` faces the viewer, so a light behind the
// surface has `dot(N, L) < 0`; `radiance` is the light shadowed at the back face;
// `transmission` the material's colour times amount (`material().transmission.rgb`,
// read once by the caller, outside its light loop).
fn transmitted(N: vec3<f32>, L: vec3<f32>, radiance: vec3<f32>, albedo: vec3<f32>, transmission: vec3<f32>) -> vec3<f32> {
    return transmission * albedo / PI * radiance * saturate(dot(-N, L));
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
    let m = material();
    let size = vec2<f32>(textureDimensions(t_opaque));
    let eye_to_surface = distance(camera.camera_pos, world);
    var uv = pixel / size;
    if (m.refraction_model != 0u) {
        let V = normalize(camera.camera_pos - world);
        // Both faces are drawn: bend through the face toward the viewer.
        let Nf = faceForward(N, -V, N);
        let R = refract(-V, Nf, 1.0 / m.ior);
        let dist = m.thickness / max(dot(R, -Nf), 1e-5);
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
        depth = m.absorption.w;
    }
    return scene.rgb * exp(-m.absorption.rgb * depth);
}

// TAA's reactive texel for a transparent surface that drew `color` (#1845,
// `reactive.wgsl`): over the pixel by its alpha (Alpha, Premultiply, and a surface
// over the opaque copy, which writes alpha 1), adding its light (Additive: times
// alpha unless premultiplied), or multiplying it (Multiply); a material's `Off` or
// `Full` replaces it. Written by `fs_transparent` where the view runs TAA.
fn reactive_texel(color: vec4<f32>) -> vec4<f32> {
    let m = material();
    var r = reactive::over(color.a);
    if (m.blend_mode == 2u) {
        let premultiplied = m.premultiply_diffuse == 1u || m.uses_opaque == 1u;
        r = reactive::added(select(color.rgb * color.a, color.rgb, premultiplied));
    } else if (m.blend_mode == 3u) {
        r = reactive::multiplied(color.rgb);
    }
    return reactive::texel(r, m.reactive, camera.stack.x);
}

// What `fs_transparent` writes: the colour, and its reactive texel.
struct ReactiveTargets {
    @location(0) color: vec4<f32>,
    @location(1) reactive: vec4<f32>,
};

fn reactive_targets(color: vec4<f32>) -> ReactiveTargets {
    return ReactiveTargets(color, reactive_texel(color));
}
