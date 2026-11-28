//! An additive scene's library meshes merge by name with the prefab's rule for materials.

use glam::Vec3;

use super::tests::{join, piece, running};
use crate::authoring::{remove_component, ComponentKind};
use rusty_components::mesh::MeshData;
use rusty_components::ColliderComponent;
use rusty_scene::mesh_library::kept_mesh_name;
use rusty_scene::Scene;

fn triangle(size: f32) -> MeshData {
    MeshData {
        positions: vec![[0.0; 3], [size, 0.0, 0.0], [0.0, size, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    }
}

/// `name` draws the library mesh `"Wall"`, and `"{name} kept"` collides with its kept copy alone.
fn wall(scene: &mut Scene, name: &str, size: f32) {
    scene.define_mesh("Wall", triangle(size)).unwrap();
    for name in [name.to_string(), format!("{name} kept")] {
        let id = scene.add_entity(name);
        crate::mesh::set_shared_mesh(scene, id, "Wall").unwrap();
        let collider = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
        scene.world.set_collider(id, Some(collider));
    }
    let kept = scene.find_entity_by_name(&format!("{name} kept")).unwrap();
    remove_component(scene, kept, ComponentKind::Mesh);
}

/// What `name` draws (name, first vertex's x) and what `"{name} kept"` collides with.
fn drawn(scene: &Scene, name: &str) -> (Option<String>, f32, Option<String>, f32) {
    let id = |n: &str| scene.find_entity_by_name(n).unwrap();
    let mesh = scene.world.mesh(id(name)).unwrap();
    let kept = scene.world.collider(id(&format!("{name} kept"))).unwrap();
    let kept_x = kept.shared_mesh.as_ref().unwrap().positions[1][0];
    let kept_name = kept_mesh_name(&kept).map(str::to_string);
    (
        mesh.asset_ref.clone(),
        mesh.vertices[1].position[0],
        kept_name,
        kept_x,
    )
}

#[test]
fn a_library_mesh_of_a_taken_name_is_renamed_and_its_users_follow() {
    let mut scene = running();
    wall(&mut scene, "Base", 1.0);
    let mut incoming = piece();
    wall(&mut incoming, "Piece", 2.0);
    join(&mut scene, &incoming);
    let w = |s: &str| Some(s.to_string());
    let theirs = (w("Wall (1)"), 2.0, w("Wall (1)"), 2.0);
    assert_eq!(
        drawn(&scene, "Piece"),
        theirs,
        "its own geometry, colliders too"
    );
    let base = (w("Wall"), 1.0, w("Wall"), 1.0);
    assert_eq!(drawn(&scene, "Base"), base, "base's untouched");
    let mut twin = piece();
    wall(&mut twin, "Twin", 1.0);
    join(&mut scene, &twin);
    assert_eq!(scene.meshes.len(), 2, "an equal mesh is shared, not copied");
    assert_eq!(drawn(&scene, "Twin"), base);
}
