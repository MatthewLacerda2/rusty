// The velocity target's texel, which `shader.wgsl`'s motion-vector entry points
// (`fs_velocity`, `fs_velocity_ssr`, `fs_depth_postpass`) write at location 1. It sits apart
// from `shader.wgsl` for that file's size cap.

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
        // `clip_now` went through the jittered projection (`stack.zw`, TAA's):
        // the jitter comes back out, so a still surface reads 0.
        velocity = clip_now.xy / clip_now.w - camera.stack.zw - clip_prev.xy / clip_prev.w;
    }
    // Depth along this camera's view axis, not the straight-line distance: a plane
    // facing the camera has one depth, so depth of field focuses on a plane.
    let depth = dot(camera.clusters.view_z.xyz, world) + camera.clusters.view_z.w;
    return vec4<f32>(velocity, depth, camera.stack.x);
}
