use crate::create::{primitive_mesh_component, Primitive};
use crate::Scene;
use rusty_components::mesh::{BoneBinding, SCENE_KIND};
use rusty_components::MeshComponent;

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
