//! The limits every device is created with: wgpu's defaults, but for the sampled
//! textures one shader stage may bind (#1703).
//!
//! The forward pass's fragment stage binds 19: the 16 of wgpu's default (the sky
//! and probe cubes, the decal atlas twice, the lightmap pages twice, the material
//! maps, the cascades, the AO, the shadow atlas and the opaque copy, #1750), the
//! integrated volumetric fog (#1703), the shadowmask pages (#1695) and the
//! material's detail map (#1997). Every adapter rusty ships on has room: Metal
//! allows 96 sampled textures per stage on macOS GPUs and Apple6 or later, 72 on
//! Apple4 and Apple5 (wgpu-hal's `metal/adapter.rs`), desktop Vulkan drivers and
//! lavapipe far more. Asking for exactly what is bound costs nothing (it is a
//! validation limit, not an allocation) and fails the device loudly on an adapter
//! that cannot draw the forward pass, rather than the pipeline later.
//!
//! Samplers are the tighter budget: Metal binds at most 16 per stage, a hard cap
//! no limit request lifts, and the forward fragment stage uses 14. The tests count
//! both from the bind-group layouts themselves.

/// The features a device takes where its adapter has them: BC texture
/// compression (#1574) and GPU occlusion culling's indirect draws (#1963). Each
/// has a path without it.
pub(crate) const OPTIONAL: wgpu::Features =
    crate::render::gpu::textures::compressed::FEATURE.union(crate::render::passes::hiz::FEATURE);

/// The most sampled textures one stage binds: the forward fragment stage's.
pub(crate) const SAMPLED_TEXTURES_PER_STAGE: u32 = 19;

/// What [`super::Renderer`]'s device asks for.
pub(crate) fn device_limits() -> wgpu::Limits {
    let defaults = wgpu::Limits::default();
    wgpu::Limits {
        max_sampled_textures_per_shader_stage: defaults
            .max_sampled_textures_per_shader_stage
            .max(SAMPLED_TEXTURES_PER_STAGE),
        ..defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::gpu::bind_layouts as layouts;

    /// Metal's samplers per shader stage, a hard cap (argument buffers aside).
    const METAL_SAMPLERS_PER_STAGE: u32 = 16;

    #[test]
    fn the_forward_stages_textures_fit_and_nothing_else_moves() {
        let (limits, defaults) = (device_limits(), wgpu::Limits::default());
        assert_eq!(limits.max_sampled_textures_per_shader_stage, 19);
        let raised = wgpu::Limits {
            max_sampled_textures_per_shader_stage: 19,
            ..defaults
        };
        assert_eq!(limits, raised);
    }

    #[test]
    fn the_forward_fragment_stage_fits_metals_samplers_and_the_textures_asked_for() {
        let groups = [
            layouts::camera_lighting_entries(),
            layouts::entity_bones_entries(),
            layouts::material_entries(),
            layouts::shadow_entries(),
        ];
        let fragment = groups.iter().flatten();
        let fragment = fragment.filter(|e| e.visibility.contains(wgpu::ShaderStages::FRAGMENT));
        let (mut samplers, mut textures) = (0, 0);
        for entry in fragment {
            match entry.ty {
                wgpu::BindingType::Sampler(_) => samplers += 1,
                wgpu::BindingType::Texture { .. } => textures += 1,
                _ => {}
            }
        }
        assert!(
            samplers <= METAL_SAMPLERS_PER_STAGE,
            "the forward fragment stage binds {samplers} samplers; Metal allows {METAL_SAMPLERS_PER_STAGE}"
        );
        assert!(
            textures <= SAMPLED_TEXTURES_PER_STAGE,
            "the forward fragment stage binds {textures} sampled textures; \
             the device asks for {SAMPLED_TEXTURES_PER_STAGE} (raise it, and check Metal's 72)"
        );
        assert_eq!((samplers, textures), (14, 19), "the counts the docs state");
    }
}
