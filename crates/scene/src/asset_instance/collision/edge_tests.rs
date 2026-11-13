//! A collision mesh (#1960) at its edges, from the second review: it starts with
//! one subdivision (no "held at" warning on any rebuild), an undo refits the box to
//! it once every mesh is back, and collision meshes that name each other hide nothing.

use super::tests::{chair, spawn, triangles};
use crate::asset_instance::instantiate_asset;
use crate::authoring::rendering::mesh::hold_subdivisions;
use crate::authoring::{add_component, remove_component, ComponentKind};
use crate::serialize::to_scene_data;
use crate::Scene;
use rusty_asset::{sidecar, MeshColliderKind, SubObjectCollider};

/// Entity `id`'s collision mesh has one subdivision, so a rebuild warns of nothing.
fn starts_at_one(scene: &Scene, id: u32) {
    let c = scene.world.collider(id).unwrap();
    let shared = c.collision_mesh.as_ref().expect("a collision mesh");
    assert_eq!(shared.subdivisions, 1);
    assert_eq!(
        hold_subdivisions(&mut shared.reference()),
        None,
        "no warning"
    );
    let text = serde_json::to_string(&to_scene_data(scene)).unwrap();
    assert!(!text.contains("\"subdivisions\":0"), "{text}");
}

#[test]
fn a_spawn_with_a_collision_mesh_warns_of_no_subdivisions() {
    let path = chair("rusty_1960_chair_subdiv.obj", 3, Some("Chair_col"));
    let (scene, kids) = spawn(&path);
    starts_at_one(&scene, kids[0].1);
    let mut lone = Scene::new();
    let part = format!("{path}::Chair");
    let id = instantiate_asset(&mut lone, &part, None, glam::Vec3::ZERO).unwrap();
    starts_at_one(&lone, id);
    let mut set = Scene::new();
    let id = instantiate_asset(&mut set, &part, None, glam::Vec3::ZERO).unwrap();
    super::set_mesh_shape(&mut set, id, false, Some("Chair_col")).unwrap();
    starts_at_one(&set, id);
}

#[test]
fn undoing_an_added_mesh_refits_the_box_to_the_collision_mesh() {
    let path = chair("rusty_1960_chair_undo_add.obj", 4, Some("Chair_col"));
    let (mut scene, kids) = spawn(&path);
    let id = kids[0].1;
    assert!(remove_component(&mut scene, id, ComponentKind::Mesh));
    scene.clear_undo();
    scene.begin_edits();
    assert!(add_component(&mut scene, id, ComponentKind::Mesh));
    scene.commit_edits(None);
    assert!(scene.undo().is_some());
    assert!(scene.world.mesh(id).is_none(), "the Box is gone");
    let c = scene.world.collider(id).unwrap();
    assert!(c.shared_mesh.is_some(), "the dense Chair is kept again");
    assert_eq!(c.aabb_max.y, 0.5, "the box is Chair_col's, not the Chair's");
    assert_eq!(triangles(&scene, id), 2);
}

#[test]
fn collision_meshes_that_name_each_other_hide_nothing() {
    let path = chair("rusty_1960_chair_loop.obj", 3, Some("Chair_col"));
    let source = std::path::Path::new(&path);
    let mut settings = sidecar::load(source).unwrap();
    let back = SubObjectCollider {
        kind: MeshColliderKind::TriMesh,
        mesh: Some("Chair".into()),
    };
    settings.colliders.insert("Chair_col".into(), back);
    sidecar::save(source, &settings).unwrap(); // a hand edit: save checks nothing
    let read = sidecar::load(source).unwrap();
    assert!(read.colliders.values().all(|c| c.mesh.is_none()));
    let (scene, kids) = spawn(&path);
    let names: Vec<&str> = kids.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Chair", "Chair_col"], "both are drawn");
    assert_eq!(triangles(&scene, kids[0].1), 3 * 3 * 2, "its own mesh");
}
