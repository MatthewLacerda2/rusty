pub(crate) const OPTIONAL: wgpu::Features =
    crate::gpu::textures::compressed::FEATURE.union(crate::passes::hiz::FEATURE);

// lint: allow(gpu-default-only) only the window's device takes it; every GPU test renders headless, which never does
pub(crate) const WINDOW: wgpu::Features = match cfg!(feature = "dev") {
    true => OPTIONAL.union(wgpu::Features::TIMESTAMP_QUERY),
    false => OPTIONAL,
};

pub(crate) const SAMPLED_TEXTURES_PER_STAGE: u32 = 23;

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
    use crate::gpu::bind_layouts as layouts;

    /// Metal's samplers per shader stage, a hard cap (argument buffers aside).
    const METAL_SAMPLERS_PER_STAGE: u32 = 16;

    #[test]
    fn the_forward_stages_textures_fit_and_nothing_else_moves() {
        let (limits, defaults) = (device_limits(), wgpu::Limits::default());
        assert_eq!(limits.max_sampled_textures_per_shader_stage, 23);
        let raised = wgpu::Limits {
            max_sampled_textures_per_shader_stage: 23,
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
        assert_eq!((samplers, textures), (15, 23), "the counts the docs state");
    }
}
