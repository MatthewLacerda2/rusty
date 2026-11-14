//! crates/edit/src/mesh.rs — point an entity's Mesh at a shared mesh
//! (`Mesh.SetSharedMesh`, the inspector's mesh field): a primitive or a library
//! mesh (`rusty_scene::mesh_library`). It adds a Mesh through the authoring op when the
//! entity has none, so it sits on the edit side.

use crate::authoring::ComponentKind;
use rusty_components::mesh::{BoneBinding, SCENE_KIND};
use rusty_components::MeshComponent;
use rusty_scene::create::{primitive_mesh_component, Primitive};
use rusty_scene::Scene;

/// Point entity `id`'s Mesh at `reference`: a primitive's name (`"Box"`, …) or a
/// library mesh's, adding a Mesh when it has none (Unity's
/// `MeshFilter.sharedMesh`). The Mesh's own settings (shadows, culling, Scale In
/// Lightmap) stay; its geometry, rig and grid are the new mesh's.
pub fn set_shared_mesh(scene: &mut Scene, id: u32, reference: &str) -> Result<(), String> {
    if !scene.world.contains(id) {
        return Err(format!("no entity {id}"));
    }
    let fresh = match Primitive::parse(reference) {
        Some(p) => primitive_mesh_component(p)
            .ok_or_else(|| format!("{reference:?} is a light, not a mesh"))?,
        // The entry's own build and revision: the other entities drawing it
        // stay as they are, and nothing is built again.
        None if scene.meshes.contains_key(reference) => {
            let entry = &scene.meshes[reference];
            let (vertices, indices) = entry.geometry().clone();
            MeshComponent {
                asset_ref: Some(reference.to_string()),
                revision: entry.revision,
                ..MeshComponent::new(SCENE_KIND, vertices, indices)
            }
        }
        None => {
            return Err(format!(
                "no mesh named {reference:?} (a primitive, or one made with Mesh.Create)"
            ))
        }
    };
    if scene.world.mesh(id).is_none() {
        crate::authoring::add_component(scene, id, ComponentKind::Mesh);
    }
    if let Some(mut mesh) = scene.world.mesh_mut(id) {
        *mesh = MeshComponent {
            cast_shadows: mesh.cast_shadows,
            scale_in_lightmap: mesh.scale_in_lightmap,
            max_draw_distance: mesh.max_draw_distance,
            min_screen_size: mesh.min_screen_size,
            skeleton: BoneBinding::default(),
            ..fresh
        };
    }
    rusty_scene::mesh_library::refit(scene, id);
    Ok(())
}
