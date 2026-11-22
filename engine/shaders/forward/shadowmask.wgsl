// Unity's `MixRealtimeAndBakedShadows`: under `Shadowmask` (`lighting.shadowmask` 1)
// the realtime map holds dynamic casters only, faded out past the shadow distance, and
// the darker of the two wins; under `DistanceShadowmask` (2) it holds every caster and
// fades into the baked one past the distance. A light with no channel, or a scene
// masking nothing (0), keeps its realtime shadow.

#define_import_path forward::shadowmask

#import common::LocalLight
#import forward::bindings::{lighting, objects, s_linear_clamp, t_lightmap_masks}
#import forward::local_shadow::local_shadow_parts

// All ones while the shadowmask pages are not bound (loading or failed) or the scene
// masks nothing, for a uniform branch and no texture read. Taken before any branch
// (derivatives).
fn baked_occlusion(uv: vec2<f32>, page: u32, lod: f32, object: u32) -> vec4<f32> {
    if (lighting.shadowmask < 0.5) {
        return vec4<f32>(1.0);
    }
    if (page > 0u) {
        if (lighting.masks_bound > 0.5) {
            return textureSampleLevel(t_lightmap_masks, s_linear_clamp, uv, page - 1u, lod);
        }
        return vec4<f32>(1.0);
    }
    return objects[object].occlusion;
}

fn channel_of(baked: vec4<f32>, channel: f32) -> f32 {
    let c = u32(clamp(channel, 0.0, 3.0));
    return baked[c];
}

fn masked(channel: f32) -> bool {
    return channel >= 0.0 && lighting.shadowmask > 0.5;
}

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

fn directional_shadows(slot: u32, front: vec2<f32>, back: vec2<f32>, channel: f32, baked: vec4<f32>) -> vec2<f32> {
    let unshadowed = vec2<f32>(1.0, 1.0);
    let f = select(unshadowed, front, slot == 0u);
    let b = select(unshadowed, back, slot == 0u);
    return vec2<f32>(sun_shadow(f, channel, baked), sun_shadow(b, channel, baked));
}

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
