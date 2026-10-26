//! GPU resource construction for the forward renderer: bind-group layouts,
//! pipelines, shader registry, uniform layouts, textures, mesh vertex layout,
//! and the per-frame instancing buffers + material cache (#470). Grouped here so the render root
//! lists subsystems, not individual resource files.

/// The format of every view depth target and of each pipeline that draws into one
/// (forward, prepass, skybox, particles, ribbons, world UI). Shadow maps keep their
/// own format in `passes::shadows`.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// One bind-group entry: `resource` at `binding`. The passes that build their groups
/// entry by entry (depth of field, motion blur) share it.
pub(crate) fn bind_entry(
    binding: u32,
    resource: wgpu::BindingResource<'_>,
) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

pub(crate) mod bind_layouts;
pub(crate) mod draw_buffers;
pub(crate) mod fullscreen;
pub(crate) mod global_group;
pub(crate) mod grow_buffer;
pub(crate) mod layout_entries;
pub(crate) mod lightmaps;
pub(crate) mod material_cache;
pub mod mesh;
pub(crate) mod pipelines;
pub(crate) mod shaders;
pub(crate) mod textures;
pub(crate) mod uniforms;

#[cfg(test)]
#[path = "mesh_id_tests.rs"]
mod mesh_id_tests;

#[cfg(test)]
mod uniform_layout_tests;

#[cfg(test)]
#[path = "lightmaps_tests.rs"]
mod lightmaps_tests;
