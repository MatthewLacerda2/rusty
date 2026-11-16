//! crates/scene/src/mesh_library/share.rs — point an entity's Mesh at a shared mesh:
//! a primitive or a library mesh, Unity's `MeshFilter.sharedMesh`. The authoring op
//! (`edit::mesh::set_shared_mesh`, `Mesh.SetSharedMesh` and the inspector's mesh
//! field) resolves and writes through these, adding a Mesh through its own op first
//! when the entity has none.

use crate::create::{primitive_mesh_component, Primitive};
use crate::Scene;
use rusty_components::mesh::{BoneBinding, SCENE_KIND};
use rusty_components::MeshComponent;

/// The Mesh `reference` names: a primitive's (`"Box"`, …) or a library mesh's. A
/// library mesh keeps its entry's build and revision, so the other entities drawing
/// it stay as they are and nothing is built again.
pub fn resolve_shared(scene: &Scene, reference: &str) -> Result<MeshComponent, String> {
    match Primitive::parse(reference) {
        Some(p) => primitive_mesh_component(p)
            .ok_or_else(|| format!("{reference:?} is a light, not a mesh")),
        None if scene.meshes.contains_key(reference) => {
            let entry = &scene.meshes[reference];
            let (vertices, indices) = entry.geometry().clone();
            Ok(MeshComponent {
                asset_ref: Some(reference.to_string()),
                revision: entry.revision,
                ..MeshComponent::new(SCENE_KIND, vertices, indices)
            })
        }
        None => Err(format!(
            "no mesh named {reference:?} (a primitive, or one made with Mesh.Create)"
        )),
    }
}

/// Give entity `id` the geometry, rig and grid of `fresh`: its Mesh's own settings
/// (shadows, culling, Scale In Lightmap) stay; an entity with no Mesh takes `fresh`
/// whole. Its collider refits to the new mesh.
pub fn put_shared(scene: &mut Scene, id: u32, fresh: MeshComponent) {
    if scene.world.mesh(id).is_none() {
        scene.world.set_mesh(id, Some(fresh));
    } else if let Some(mut mesh) = scene.world.mesh_mut(id) {
        *mesh = MeshComponent {
            cast_shadows: mesh.cast_shadows,
            scale_in_lightmap: mesh.scale_in_lightmap,
            max_draw_distance: mesh.max_draw_distance,
            min_screen_size: mesh.min_screen_size,
            skeleton: BoneBinding::default(),
            ..fresh
        };
    }
    super::refit(scene, id);
}
