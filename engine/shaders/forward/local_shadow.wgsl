// engine/shaders/forward/local_shadow.wgsl — a point or spot light's shadow in the
// forward pass (#468): its atlas tile, PCF, and the per-light shadow fade distance
// (#1712). `fs_main` in `shader.wgsl` multiplies each local light by it, through
// `forward::shadowmask` (#1695).

#define_import_path forward::local_shadow

#import common::{LocalLight, NO_SHADOW, local_shadow_tile, sample_local_shadow}
#import forward::bindings::{camera, s_shadow, shadow_tiles, t_shadow_atlas}

// How much of `light`'s shadow the camera samples (#1712): all of it within nine
// tenths of its fade distance from the light, falling linearly to none at it
// (HDRP's linear distance fade); all of it when the fade is off (0).
fn shadow_fade(light: LocalLight) -> f32 {
    if (light.shadow_fade <= 0.0) {
        return 1.0;
    }
    let d = distance(camera.camera_pos, light.position);
    return clamp((light.shadow_fade - d) / (0.1 * light.shadow_fade), 0.0, 1.0);
}

// How lit `world` (normal `N`) is by `light`, in two parts: the atlas's PCF
// visibility (x) and how much of it the camera samples (y, the fade). With no tile
// or past the fade it is (1, 0) and costs no tap (the fade is uniform per light).
// `forward::shadowmask::local_mixed_shadow` blends x toward lit by y, and under a
// shadowmask meets the baked shadow there (#1695).
fn local_shadow_parts(light: LocalLight, world: vec3<f32>, N: vec3<f32>) -> vec2<f32> {
    let tile = local_shadow_tile(light, world);
    if (tile == NO_SHADOW) {
        return vec2<f32>(1.0, 0.0);
    }
    let fade = shadow_fade(light);
    if (fade <= 0.0) {
        return vec2<f32>(1.0, 0.0);
    }
    let lit = sample_local_shadow(shadow_tiles[tile], t_shadow_atlas, s_shadow, light.position, world, N);
    return vec2<f32>(lit, fade);
}
