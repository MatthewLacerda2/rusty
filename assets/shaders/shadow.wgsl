struct ShadowUniforms {
    light_space: mat4x4<f32>,
};

struct EntityUniforms {
    model_matrix: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> global: ShadowUniforms;

@group(1) @binding(0)
var<uniform> entity: EntityUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> @builtin(position) vec4<f32> {
    return global.light_space * entity.model_matrix * vec4<f32>(model.position, 1.0);
}
