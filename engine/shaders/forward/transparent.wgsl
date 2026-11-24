
#define_import_path forward::transparent

#import forward::bindings::{camera, material, s_linear_clamp, s_opaque, t_fog_volume, t_opaque}
#import forward::ssr_rays::traced_ray
#import common::{PI, fog_factor}
#import fog::volumetric::volumetric_fog
#import lighting::ssr_march
#import lighting::ssr_surface
#import reactive

fn volumetric(world: vec3<f32>) -> vec4<f32> {
    return volumetric_fog(t_fog_volume, s_linear_clamp, camera.volumetric, camera.view_proj, camera.clusters.view_z, world);
}

fn fogged(lit: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let fog = fog_factor(camera.fog, world, camera.camera_pos);
    return blended_air(mix(lit, camera.fog.color, fog), volumetric(world), 1.0);
}

// What SSR's record and SSIL's realtime share are dimmed by, so neither adds light the
// fog took. Exactly `fog_factor` where the camera reads no volume.
fn surface_fog(world: vec3<f32>) -> f32 {
    let fog = fog_factor(camera.fog, world, camera.camera_pos);
    if (camera.volumetric.x < 0.5) {
        return fog;
    }
    return 1.0 - (1.0 - fog) * volumetric(world).a;
}

// Alpha and Premultiply replace what is behind by `coverage`, so they take the air's
// light in front. Additive is only dimmed. Multiply fades toward white through the air,
// so a white sheet stays invisible.
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

// Preserve specular (URP's `_ALPHAPREMULTIPLY_ON`) blends premultiplied; opacity is
// URP's `alpha * oneMinusReflectivity + reflectivity`. A surface over the opaque copy
// writes alpha 1.
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
        color = lit * alpha;
    }
    color = mix(color, camera.fog.color * opacity, fog);
    color = blended_air(color, air, opacity);
    if (m.uses_opaque == 0u) {
        return vec4<f32>(color, opacity);
    }
    return vec4<f32>(color + (1.0 - opacity) * behind(world, N, pixel), 1.0);
}

// Under `Rgba16Float`'s largest finite 65504, with room for rounding to f16.
const MAX_HDR: f32 = 60000.0;

// Inverse-square light has no ceiling: an `Inf` would turn bloom's bright pass NaN and
// spread a black block.
fn finite_hdr(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(clamp(c.rgb, vec3<f32>(-MAX_HDR), vec3<f32>(MAX_HDR)), c.a);
}

// HDRP's *Receive SSR Transparent*: marches the opaque copy (after SSR, with each
// pixel's eye distance), so it needs no prepass. The copy has no mip pyramid, so the
// hit is sharp: *Minimum Smoothness* is never below `SHARP_SMOOTHNESS`.
fn reflect_ssr(reflection: vec3<f32>, weight: vec3<f32>, world: vec3<f32>, N: vec3<f32>, R: vec3<f32>, roughness: f32, pixel: vec2<f32>) -> vec3<f32> {
    let fade = ssr_surface::smoothness_fade(roughness, camera.ssr.z, camera.ssr.w);
    if (material().receive_ssr == 0u || camera.ssr.x < 1.0 || fade <= 0.0) {
        return reflection;
    }
    let f = ssr_march::Frame(camera.view_proj, camera.camera_pos, vec3<f32>(0.0), vec4<f32>(0.0, 0.0, 0.0, 1.0));
    let origin = world + faceForward(N, -R, N) * 0.02;
    let hit = ssr_march::trace(f, t_opaque, t_opaque, s_opaque, origin, R, i32(camera.ssr.x), camera.ssr.y, 0.0);
    var colour = hit.rgb;
    var k = hit.w;
    let ray = traced_ray(world, N, pixel);
    if (hit.w < 1.0 && ray.a > 0.0) {
        let share = (1.0 - hit.w) * ray.a;
        k = hit.w + share;
        colour = (hit.rgb * hit.w + ray.rgb * share) / k;
    }
    k = k * fade;
    return reflection + (weight * colour - reflection) * k;
}

fn transmitted(N: vec3<f32>, L: vec3<f32>, radiance: vec3<f32>, albedo: vec3<f32>, transmission: vec3<f32>) -> vec3<f32> {
    return transmission * albedo / PI * radiance * saturate(dot(-N, L));
}

// Under `opaque_copy.wgsl`'s `SKY_DISTANCE` (65504, the sky's eye distance), with room for
// rounding.
const SKY_DISTANCE: f32 = 60000.0;

// HDRP's `RefractionModelBox` refraction and Beer-Lambert absorption (`docs/api/Material.md`):
// the ray leaves the slab parallel to the view ray.
fn behind(world: vec3<f32>, N: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    let m = material();
    let size = vec2<f32>(textureDimensions(t_opaque));
    let eye_to_surface = distance(camera.camera_pos, world);
    var uv = pixel / size;
    if (m.refraction_model != 0u) {
        let V = normalize(camera.camera_pos - world);
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

// A surface over the opaque copy is Alpha or Premultiply (`MaterialAsset::reads_opaque`), so
// it is `over` too.
fn reactive_texel(color: vec4<f32>) -> vec4<f32> {
    let m = material();
    var r = reactive::over(color.a);
    if (m.blend_mode == 2u) {
        let premultiplied = m.premultiply_diffuse == 1u;
        r = reactive::added(select(color.rgb * color.a, color.rgb, premultiplied));
    } else if (m.blend_mode == 3u) {
        r = reactive::multiplied(color.rgb);
    }
    return reactive::texel(r, m.reactive, camera.stack.x);
}

struct ReactiveTargets {
    @location(0) color: vec4<f32>,
    @location(1) reactive: vec4<f32>,
};

fn reactive_targets(color: vec4<f32>) -> ReactiveTargets {
    return ReactiveTargets(color, reactive_texel(color));
}
