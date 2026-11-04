// engine/shaders/forward/motion.wgsl — the velocity target's texel (#1435): what
// `shader.wgsl`'s motion-vector entry points (`fs_velocity`, `fs_velocity_ssr`,
// `fs_depth_postpass`) write at location 1. Moved out of `shader.wgsl` for its size
// cap (#1702).

#define_import_path forward::motion

#import forward::bindings::camera

// The texel for a fragment at `world` whose clip position is `clip_now` this frame and
// was `clip_prev` last frame: its screen motion in NDC, its view depth and its camera's
// place in the stack.
fn motion_texel(clip_now: vec4<f32>, clip_prev: vec4<f32>, world: vec3<f32>) -> vec4<f32> {
    // A surface behind the camera last frame (a projectile past the player's head)
    // had no place on screen to move from: it writes no motion, not a huge streak.
    var velocity = vec2<f32>(0.0);
    if (clip_prev.w > 1e-4) {
        velocity = clip_now.xy / clip_now.w - clip_prev.xy / clip_prev.w;
    }
    // Depth along this camera's view axis, not the straight-line distance: a plane
    // facing the camera has one depth, so depth of field focuses on a plane (#1445).
    let depth = dot(camera.clusters.view_z.xyz, world) + camera.clusters.view_z.w;
    return vec4<f32>(velocity, depth, camera.stack.x);
}
