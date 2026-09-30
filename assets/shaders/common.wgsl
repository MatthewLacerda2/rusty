// assets/shaders/common.wgsl — shared GPU struct definitions imported by the
// forward, shadow, and skybox passes via `#import "common"`.

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
};

// Standard mesh vertex layout — position, normal, UVs, skeletal animation data
// (four joint indices + blend weights; no joint cap, #455), and the tangent basis for normal
// mapping (`xyz` unit tangent, `w` handedness sign).
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
    @location(5) tangent: vec4<f32>,
};

// A vertex's skinning matrix (#599): its four joint matrices blended by `weights`.
// Zero weights (a vertex no joint moves) skin as identity. The one skinning function
// the forward and shadow passes share, so a shadow always follows the drawn pose;
// each pass looks the four matrices up in its own copy of the frame's joint array.
fn blend_joints(
    j0: mat4x4<f32>,
    j1: mat4x4<f32>,
    j2: mat4x4<f32>,
    j3: mat4x4<f32>,
    weights: vec4<f32>,
) -> mat4x4<f32> {
    if (weights.x + weights.y + weights.z + weights.w < 0.01) {
        return mat4x4<f32>(
            vec4<f32>(1.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 1.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 1.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 1.0)
        );
    }
    return j0 * weights.x + j1 * weights.y + j2 * weights.z + j3 * weights.w;
}
