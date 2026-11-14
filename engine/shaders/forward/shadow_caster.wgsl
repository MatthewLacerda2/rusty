// engine/shaders/forward/shadow_caster.wgsl — the shadow pass's own resources
// (#435, #648): the cascade's matrix and the casters `shader.wgsl`'s `vs_shadow` and
// `shadow_clip` read. Moved out of `shader.wgsl` for its size cap (#1882).

#define_import_path forward::shadow_caster

#import forward::mapping

struct ShadowLight {
    light_space: mat4x4<f32>,
};

// The cascade being drawn: one matrix per cascade, picked by dynamic offset.
@group(0) @binding(6)
var<uniform> shadow_light: ShadowLight;

// One caster (#470): its world matrix, where its joint palette starts in `bones`
// (#599; 0, the shared identity, for a mesh with no skin), and its cutout alpha test
// (#648): the albedo map's alpha when `use_texture == 1` (a cutout's tint alpha is
// always 1), read where its material's `mapping` puts it (#1594), clipped below
// `alpha_cutoff` (0 for a material that does not clip).
// Mirrors `CasterData` (crates/render/src/passes/shadows/casters/buffer.rs) byte-for-byte.
struct Caster {
    model: mat4x4<f32>,
    bone_base: u32,
    alpha_cutoff: f32,
    use_texture: u32,
    // Padding; a module's names may not start with `_` or end in a digit (naga_oil).
    caster_pad: u32,
    // Its material's mapping (#1594); not named `mapping`, which is the module's.
    terms: mapping::Mapping,
};

// Every caster, one per instance: a draw covers a run of casters sharing one mesh
// (and material, when it clips), indexed by `instance_index`.
@group(1) @binding(3)
var<storage, read> casters: array<Caster>;
