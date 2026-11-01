//! Undo records a Mesh-shaped collider's shared mesh (#1657) as its reference, like
//! a Mesh, and puts its geometry back: the live one's on an edit, rebuilt from the
//! reference on an undone Destroy.

use glam::Vec3;

use crate::components::ColliderComponent;
use crate::scene::authoring::{create_entity, remove_component, ComponentKind, Primitive};
use crate::scene::Scene;

fn blockout() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Blockout", Some(Primitive::Box));
    let collider = ColliderComponent::from_mesh_bounds(true, Vec3::splat(-0.5), Vec3::splat(0.5));
    scene.world.set_collider(id, Some(collider));
    remove_component(&mut scene, id, ComponentKind::Mesh);
    scene.clear_undo();
    (scene, id)
}

fn shared_geometry(scene: &Scene, id: u32) -> (usize, usize) {
    let collider = scene.world.collider(id).expect("a collider");
    let shared = collider.shared_mesh.as_ref().expect("a shared mesh");
    (shared.positions.len(), shared.indices.len())
}

#[test]
fn a_record_keeps_the_reference_and_not_the_geometry() {
    let (scene, id) = blockout();
    assert_ne!(shared_geometry(&scene, id), (0, 0));
    let recorded = scene.world.record(id).expect("live");
    let shared = recorded.doc.collider.and_then(|c| c.shared_mesh);
    let shared = shared.expect("the reference is recorded");
    assert_eq!(shared.primitive_type, "Box");
    assert!(shared.positions.is_empty() && shared.indices.is_empty());
    let reference = scene.world.reference_document(id).unwrap();
    let shared = reference.collider.and_then(|c| c.shared_mesh).unwrap();
    assert!(
        shared.positions.is_empty(),
        "a prefab diff copies none either"
    );
}

#[test]
fn an_undone_edit_keeps_the_live_geometry() {
    let (mut scene, id) = blockout();
    let geometry = shared_geometry(&scene, id);
    scene.begin_edits();
    scene.world.transform_mut(id).unwrap().position = Vec3::X;
    scene.commit_edits(None);
    assert!(scene.undo().is_some());
    assert_eq!(shared_geometry(&scene, id), geometry);
    assert!(scene.redo().is_some());
    assert_eq!(shared_geometry(&scene, id), geometry);
}

#[test]
fn an_undone_destroy_rebuilds_the_shared_mesh() {
    let (mut scene, id) = blockout();
    let geometry = shared_geometry(&scene, id);
    scene.begin_edits();
    scene.destroy_entity(id);
    scene.commit_edits(None);
    assert!(scene.undo().is_some());
    assert_eq!(shared_geometry(&scene, id), geometry);
}
