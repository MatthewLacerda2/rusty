#import common::{VertexInput}

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

@vertex
fn vs_main(model: VertexInput) -> @builtin(position) vec4<f32> {
    return global.light_space * entity.model_matrix * vec4<f32>(model.position, 1.0);
}
