//! crates/edit/src/mesh.rs — point an entity's Mesh at a shared mesh
//! (`Mesh.SetSharedMesh`, the inspector's mesh field): a primitive or a library
//! mesh (`rusty_scene::mesh_library`). It adds a Mesh through the authoring op when the
//! entity has none, so it sits on the edit side; the resolve and the write are
//! `rusty_scene::mesh_library`'s.

use crate::authoring::ComponentKind;
use rusty_scene::mesh_library::{put_shared, resolve_shared};
use rusty_scene::Scene;

/// Point entity `id`'s Mesh at `reference`: a primitive's name (`"Box"`, …) or a
/// library mesh's, adding a Mesh when it has none (Unity's
/// `MeshFilter.sharedMesh`). The Mesh's own settings (shadows, culling, Scale In
/// Lightmap) stay; its geometry, rig and grid are the new mesh's.
pub fn set_shared_mesh(scene: &mut Scene, id: u32, reference: &str) -> Result<(), String> {
    if !scene.world.contains(id) {
        return Err(format!("no entity {id}"));
    }
    let fresh = resolve_shared(scene, reference)?;
    if scene.world.mesh(id).is_none() {
        crate::authoring::add_component(scene, id, ComponentKind::Mesh);
    }
    put_shared(scene, id, fresh);
    Ok(())
}
