#import common::{VertexInput}

struct ShadowUniforms {
    light_space: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> global: ShadowUniforms;

// Every caster's world matrix, one per instance (#470): a draw covers a run of
// casters sharing one mesh, indexed by `instance_index`.
@group(1) @binding(0)
var<storage, read> models: array<mat4x4<f32>>;

@vertex
fn vs_main(model: VertexInput, @builtin(instance_index) instance: u32) -> @builtin(position) vec4<f32> {
    return global.light_space * models[instance] * vec4<f32>(model.position, 1.0);
}
