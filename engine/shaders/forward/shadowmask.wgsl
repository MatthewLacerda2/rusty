// engine/shaders/forward/shadowmask.wgsl — a `Mixed` light's baked shadow under the
// scene's `Shadowmask` mode (#1695), and how it meets the realtime one.
//
// The bake stores, per shadowmask channel, how much of the `Mixed` light on it
// reaches a texel past static occluders (the shadowmask pages, beside the
// lightmaps) and a probe (`ObjectData::occlusion`, interpolated at the object). A
// light names its channel (`mask_channel`, -1 for none). Its direct light (diffuse
// and specular) stays live; only its static shadow is baked, so it holds at any
// distance and costs no shadow map there.
//
// The realtime and baked shadows meet as in Unity's `MixRealtimeAndBakedShadows`:
// under `Shadowmask` (`lighting.shadowmask` 1) the realtime map holds dynamic
// casters only, faded out past the shadow distance, and the darker of the two wins;
// under `DistanceShadowmask` (2) it holds every caster and fades into the baked one
// past the distance. A light that names no channel, and every light while the
// scene masks nothing (0), keeps its realtime shadow exactly as before.

#define_import_path forward::shadowmask

#import common::LocalLight
#import forward::bindings::{lighting, objects, s_lightmaps, t_lightmap_masks}
#import forward::local_shadow::local_shadow_parts

// The pixel's baked occlusion per channel: a lightmapped surface's shadowmask
// texel (page `page` + 1, at `uv` and mip `lod`, taken before any branch), or all
// ones while its pages are not bound (still loading, or failed: the realtime
// shadow alone, whose maps then keep their static casters, as the renderer's
// `masking_statics` holds, so the surface draws as Distance Shadowmask does near
// the camera); anything else reads the probes' (`objects[object].occlusion`: a
// dynamic object, a static one with no lightmap). All ones while the scene masks
// nothing, for a uniform branch and no texture read.
fn baked_occlusion(uv: vec2<f32>, page: u32, lod: f32, object: u32) -> vec4<f32> {
    if (lighting.shadowmask < 0.5) {
        return vec4<f32>(1.0);
    }
    if (page > 0u) {
        if (lighting.masks_bound > 0.5) {
            return textureSampleLevel(t_lightmap_masks, s_lightmaps, uv, page - 1u, lod);
        }
        return vec4<f32>(1.0);
    }
    return objects[object].occlusion;
}

// Channel `channel` of `baked`, or none (-1).
fn channel_of(baked: vec4<f32>, channel: f32) -> f32 {
    let c = u32(clamp(channel, 0.0, 3.0));
    return baked[c];
}

// Whether `channel` masks under this scene: a channel, and a scene that masks.
fn masked(channel: f32) -> bool {
    return channel >= 0.0 && lighting.shadowmask > 0.5;
}

// The sun's (or any directional light's) shadow: `parts` its realtime shadow and
// how far it has faded out (`forward::lighting::sun_shadow_parts`), mixed with
// channel `channel` of `baked`. Unmasked, exactly what `calculate_shadow` gives.
fn sun_shadow(parts: vec2<f32>, channel: f32, baked: vec4<f32>) -> f32 {
    let faded = mix(parts.x, 1.0, parts.y);
    if (!masked(channel)) {
        return faded;
    }
    let mask = channel_of(baked, channel);
    if (lighting.shadowmask > 1.5) {
        return mix(parts.x, mask, parts.y);
    }
    return min(faded, mask);
}

// A point or spot light's shadow: `parts` its realtime shadow and how much of it
// the camera samples (`forward::local_shadow::local_shadow_parts`: 0 with no atlas
// tile, past its fade distance or off a surface that receives none), mixed with
// channel `channel` of `baked`. Unmasked, exactly what `local_shadow` gives.
fn local_mixed_shadow(parts: vec2<f32>, channel: f32, baked: vec4<f32>) -> f32 {
    let faded = mix(1.0, parts.x, parts.y);
    if (!masked(channel)) {
        return faded;
    }
    let mask = channel_of(baked, channel);
    if (lighting.shadowmask > 1.5) {
        return mix(mask, parts.x, parts.y);
    }
    return min(faded, mask);
}

// Directional slot `slot`'s shadow off the front and back faces: the sun's (slot
// 0, the one with cascades) from its parts `front` and `back`, any other unshadowed
// but for its mask.
fn directional_shadows(slot: u32, front: vec2<f32>, back: vec2<f32>, channel: f32, baked: vec4<f32>) -> vec2<f32> {
    let unshadowed = vec2<f32>(1.0, 1.0);
    let f = select(unshadowed, front, slot == 0u);
    let b = select(unshadowed, back, slot == 0u);
    return vec2<f32>(sun_shadow(f, channel, baked), sun_shadow(b, channel, baked));
}

// A point or spot light's shadow at `world` off the front face (x) and, on a
// `translucent` surface, the back (y, off `-N`): its atlas shadow where the surface
// `receives` one, met with its baked shadow.
fn local_shadows(light: LocalLight, world: vec3<f32>, N: vec3<f32>, receives: bool, translucent: bool, baked: vec4<f32>) -> vec2<f32> {
    var front = vec2<f32>(1.0, 0.0);
    var back = vec2<f32>(1.0, 0.0);
    if (receives) {
        front = local_shadow_parts(light, world, N);
        if (translucent) {
            back = local_shadow_parts(light, world, -N);
        }
    }
    return vec2<f32>(local_mixed_shadow(front, light.mask_channel, baked), local_mixed_shadow(back, light.mask_channel, baked));
}
