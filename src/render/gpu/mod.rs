//! GPU resource construction for the forward renderer: bind-group layouts,
//! pipelines, shader registry, uniform layouts, textures, mesh vertex layout,
//! and the per-frame instancing buffers + material cache (#470). Grouped here so the render root
//! lists subsystems, not individual resource files.

pub(crate) mod bind_layouts;
pub(crate) mod draw_buffers;
pub(crate) mod grow_buffer;
pub(crate) mod material_cache;
pub mod mesh;
pub(crate) mod pipelines;
pub(crate) mod shaders;
pub(crate) mod textures;
pub(crate) mod uniforms;

#[cfg(test)]
#[path = "mesh_id_tests.rs"]
mod mesh_id_tests;
