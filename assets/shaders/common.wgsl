// assets/shaders/common.wgsl — shared GPU struct definitions imported by the
// forward, shadow, and skybox passes via `#import "common"`.

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
};

// Standard mesh vertex layout — position, normal, UVs, skeletal animation data
// (joint indices + blend weights, 64 bones max), and the tangent basis for normal
// mapping (`xyz` unit tangent, `w` handedness sign).
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
    @location(5) tangent: vec4<f32>,
};
