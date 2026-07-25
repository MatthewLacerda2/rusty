//! GPU resource construction for the forward renderer: bind-group layouts,
//! pipelines, shader registry, uniform layouts, textures, tangents, mesh upload,
//! and the persistent per-entity buffer pool. Grouped here so the render root
//! lists subsystems, not individual resource files.

pub(crate) mod bind_layouts;
pub(crate) mod entity_pool;
pub mod mesh;
pub(crate) mod pipelines;
pub(crate) mod shaders;
pub(crate) mod slot_key;
pub(crate) mod tangents;
pub(crate) mod textures;
pub(crate) mod uniforms;

#[cfg(test)]
#[path = "mesh_id_tests.rs"]
mod mesh_id_tests;
