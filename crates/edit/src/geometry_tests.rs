//! What the scene document copies of a mesh: no geometry, which no reader keeps,
//! and the same saved text as a copy with it.

use glam::Vec3;

use crate::asset_instance::instantiate_asset;
use crate::to_scene_data;
use rusty_components::ColliderComponent;
use rusty_scene::Scene;
use rusty_test_support::models::quad_file;

/// One quad instance with a Mesh-shaped collider whose collision mesh is the quad.
fn level() -> Scene {
    let path = quad_file("rusty_document_geometry");
    let reference = format!("{path}::Quad");
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    let mut collider = ColliderComponent::from_mesh_bounds(false, Vec3::splat(-1.0), Vec3::ONE);
    let mut collision = rusty_components::SharedMesh::of(&scene.world.mesh(id).unwrap());
    collision.positions = vec![[0.0; 3]; 3];
    collision.indices = vec![0, 1, 2].into();
    collider.collision_mesh = Some(collision);
    scene.world.set_collider(id, Some(collider));
    scene
}

#[test]
fn a_document_carries_no_mesh_geometry() {
    let scene = level();
    let id = scene.world.ids_with_mesh()[0];
    assert!(
        !scene.world.mesh(id).unwrap().vertices.is_empty(),
        "the live quad"
    );
    let data = to_scene_data(&scene);
    let doc = data.entities.iter().find(|e| e.id == id).unwrap();
    let mesh = doc.mesh.as_ref().unwrap();
    assert!(mesh.vertices.is_empty() && mesh.indices.is_empty());
    assert_eq!(mesh.asset_ref, scene.world.mesh(id).unwrap().asset_ref);
    assert_eq!(mesh.revision, scene.world.mesh(id).unwrap().revision);
    let collision = doc
        .collider
        .as_ref()
        .unwrap()
        .collision_mesh
        .as_ref()
        .unwrap();
    assert!(collision.positions.is_empty() && collision.indices.is_empty());
}

/// The text a save writes is the one a full copy writes: the geometry is never
/// written either way.
#[test]
fn a_saved_document_is_the_text_a_full_copy_writes() {
    let scene = level();
    let full = serde_json::to_string_pretty(&scene.world.collect_entities()).unwrap();
    let saved = serde_json::to_string_pretty(&scene.world.collect_documents()).unwrap();
    assert_eq!(saved, full);
    assert!(saved.contains("collision_mesh"), "{saved}");
}
