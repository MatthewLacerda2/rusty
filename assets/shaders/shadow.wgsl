#import common::{VertexInput, blend_joints}

struct ShadowUniforms {
    light_space: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> global: ShadowUniforms;

// One caster (#470): its world matrix, and where its joint palette starts in `bones`
// (#599) — 0, the shared identity, for a mesh with no skin.
struct Caster {
    model: mat4x4<f32>,
    bone_base: u32,
};

// Every caster, one per instance: a draw covers a run of casters sharing one mesh,
// indexed by `instance_index`.
@group(1) @binding(0)
var<storage, read> casters: array<Caster>;

// The sweep's skinned casters' joint matrices back to back, from the same palettes
// the forward pass draws (#599). Element 0 is the identity.
@group(1) @binding(1)
var<storage, read> bones: array<mat4x4<f32>>;

@vertex
fn vs_main(model: VertexInput, @builtin(instance_index) instance: u32) -> @builtin(position) vec4<f32> {
    let caster = casters[instance];
    let joints = model.joint_indices + vec4<u32>(caster.bone_base);
    let skin = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    return global.light_space * caster.model * skin * vec4<f32>(model.position, 1.0);
}
