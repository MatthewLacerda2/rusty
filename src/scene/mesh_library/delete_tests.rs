//! `Scene::delete_mesh` (#2059, `Mesh.DeleteAsset`): refused while used, forced off
//! every user.

use glam::Vec3;

use super::kept_mesh_name;
use crate::components::mesh::MeshData;
use crate::components::ColliderComponent;
use crate::scene::authoring::{remove_component, ComponentKind};
use crate::scene::Scene;

fn triangle() -> MeshData {
    MeshData {
        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    }
}

/// An entity drawing `name` through a Mesh collider's kept copy alone.
pub(super) fn kept_user(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity("Kept".into());
    scene.set_shared_mesh(id, name).unwrap();
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    scene.world.set_collider(id, Some(collider));
    remove_component(scene, id, ComponentKind::Mesh);
    assert_eq!(
        kept_mesh_name(&scene.world.collider(id).unwrap()),
        Some(name)
    );
    id
}

#[test]
fn delete_refuses_a_used_mesh_unless_forced() {
    let mut scene = Scene::new();
    scene.define_mesh("plank", triangle()).unwrap();
    let drawn = scene.add_entity("Plank".into());
    scene.set_shared_mesh(drawn, "plank").unwrap();
    let kept = kept_user(&mut scene, "plank");
    let refused = scene.delete_mesh("plank", false).unwrap_err();
    assert!(
        refused.contains(&format!("entity {drawn} \"Plank\"")),
        "{refused}"
    );
    assert!(
        refused.contains(&format!("entity {kept} \"Kept\"")),
        "{refused}"
    );
    assert!(scene.meshes.contains_key("plank"), "nothing changed");
    assert_eq!(scene.delete_mesh("plank", true).unwrap(), vec![drawn, kept]);
    assert!(scene.meshes.is_empty() && scene.world.mesh(drawn).is_none());
    assert!(scene.world.collider(kept).unwrap().shared_mesh.is_none());
    assert!(scene
        .delete_mesh("plank", false)
        .unwrap_err()
        .contains("no mesh"));
    scene.define_mesh("spare", triangle()).unwrap();
    assert_eq!(
        scene.delete_mesh("spare", false).unwrap(),
        Vec::<u32>::new()
    );
}

#[test]
fn undo_of_a_forced_delete_brings_the_mesh_and_its_users_back() {
    let mut scene = Scene::new();
    scene.define_mesh("plank", triangle()).unwrap();
    let drawn = scene.add_entity("Plank".into());
    scene.set_shared_mesh(drawn, "plank").unwrap();
    scene.begin_edits();
    scene.delete_mesh("plank", true).unwrap();
    scene.commit_edits(Some("Delete"));
    assert_eq!(scene.undo().as_deref(), Some("Delete"));
    assert!(scene.meshes.contains_key("plank"));
    assert_eq!(scene.world.mesh(drawn).unwrap().vertices.len(), 3);
}

#[test]
fn a_forced_delete_empties_the_box_of_a_collider_left_with_nothing() {
    let mut scene = Scene::new();
    scene.define_mesh("plank", triangle()).unwrap();
    let drawn = scene.add_entity("Plank".into());
    let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    scene.world.set_collider(drawn, Some(collider));
    scene.set_shared_mesh(drawn, "plank").unwrap();
    assert_eq!(scene.world.collider(drawn).unwrap().aabb_max.x, 1.0);
    scene.delete_mesh("plank", true).unwrap();
    let collider = scene.world.collider(drawn).unwrap();
    assert_eq!(
        (collider.aabb_min, collider.aabb_max),
        (Vec3::ZERO, Vec3::ZERO)
    );
}
