// engine/shaders/sky/motion.wgsl — what a sky pass writes into a motion-blurred
// view's velocity target (#1435): `skybox.wgsl` and `sky_gradient.wgsl` import it.

#define_import_path sky::motion

#import common::CameraUniforms

// A sky pass's outputs: the colour, and the velocity a motion-vector view's second
// target keeps (`sky_targets`); a view without one ignores location 1.
struct SkyTargets {
    @location(0) color: vec4<f32>,
    @location(1) velocity: vec4<f32>,
};

// The sky's view distance in the velocity target (#1436): a half float's largest
// value, as far as anything gets. Mirrors `render::motion::SKY_DISTANCE`.
const SKY_DISTANCE: f32 = 65504.0;

// What a sky pass writes for `color` seen along `dir`: its colour, and its motion
// beside its distance, the sky's.
fn sky_targets(camera: CameraUniforms, color: vec4<f32>, dir: vec3<f32>) -> SkyTargets {
    return SkyTargets(color, vec4<f32>(sky_velocity(camera, dir), SKY_DISTANCE, 0.0));
}

// The sky's motion vector along view direction `dir` (#1435): a point at infinity
// moves with the camera's rotation alone, so it reprojects as a direction (w = 0)
// through this and last frame's view-projection. An orthographic lens sees no
// direction move (w is 0 for both), so its sky reads 0.
fn sky_velocity(camera: CameraUniforms, dir: vec3<f32>) -> vec2<f32> {
    let now = camera.view_proj * vec4<f32>(dir, 0.0);
    let prev = camera.prev_view_proj * vec4<f32>(dir, 0.0);
    if (abs(now.w) < 1e-6 || abs(prev.w) < 1e-6) {
        return vec2<f32>(0.0);
    }
    return now.xy / now.w - prev.xy / prev.w;
}
