// engine/shaders/forward/local_shadow.wgsl — a point or spot light's shadow in the
// forward pass (#468): its atlas tile, PCF, and the per-light shadow fade distance
// (#1712). `fs_main` in `shader.wgsl` multiplies each local light by it.

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

// How lit `world` (normal `N`) is by `light`: 1.0 when the atlas holds no tile for
// it or its shadow has faded out, which then costs no tap (the fade is uniform per
// light), else its PCF shadow blended towards lit by the fade.
fn local_shadow(light: LocalLight, world: vec3<f32>, N: vec3<f32>) -> f32 {
    let tile = local_shadow_tile(light, world);
    let fade = shadow_fade(light);
    if (tile == NO_SHADOW || fade <= 0.0) {
        return 1.0;
    }
    let lit = sample_local_shadow(shadow_tiles[tile], t_shadow_atlas, s_shadow, light.position, world, N);
    return mix(1.0, lit, fade);
}
