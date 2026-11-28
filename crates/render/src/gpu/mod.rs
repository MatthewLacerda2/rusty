pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// How a view's depth is compared and cleared: reversed (`rusty_scene::camera::depth`), so the
/// nearer surface has the greater depth. A pass into a view target names these, never a compare
/// or clear of its own (`depth-direction`); shadow maps keep ordinary depth.
pub(crate) mod depth {
    use rusty_scene::camera::depth::FAR;

    /// Nearer than what the target holds.
    pub(crate) const NEARER: wgpu::CompareFunction = wgpu::CompareFunction::Greater;
    /// Nearer or level: a pass over depth already drawn, and the sky on the far plane.
    pub(crate) const NEARER_OR_EQUAL: wgpu::CompareFunction = wgpu::CompareFunction::GreaterEqual;
    /// The far plane: no surface drawn.
    pub(crate) const CLEAR: wgpu::LoadOp<f32> = wgpu::LoadOp::Clear(FAR);
}

#[cfg(test)]
#[path = "depth_tests.rs"]
mod depth_tests;

#[cfg(test)]
#[path = "depth_gpu_tests.rs"]
mod depth_gpu_tests;

pub(crate) fn bind_entry(
    binding: u32,
    resource: wgpu::BindingResource<'_>,
) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

pub(crate) const ONLY_GROUP: u32 = 0;

pub(crate) fn linear_clamp(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Linear Clamp Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

pub(crate) mod bind_layouts;
pub(crate) mod commands;
pub(crate) mod draw_buffers;
pub(crate) mod fullscreen;
pub(crate) mod global_group;
pub(crate) mod groups;
pub(crate) mod grow_buffer;
pub(crate) mod kept;
pub(crate) mod layout_entries;
pub(crate) mod lightmaps;
pub(crate) mod material_cache;
pub mod mesh;
pub(crate) mod ops;
pub(crate) mod pipelines;
pub(crate) mod shaders;
pub(crate) mod textures;
pub(crate) mod uniforms;

#[cfg(test)]
#[path = "mesh_id_tests.rs"]
mod mesh_id_tests;

#[cfg(test)]
pub(crate) mod uniform_layout_tests;
