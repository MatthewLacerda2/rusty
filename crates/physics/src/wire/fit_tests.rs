use glam::Vec3;

use super::{collider_wire, WireCache};
use crate::shape::fixture::arch;
use rusty_components::{RigidBodyComponent, SharedMesh};
use rusty_scene::create::defaults::default_rigidbody;
use rusty_scene::{ColliderComponent, ColliderShape, Scene};

fn arched() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("arch".to_string());
    let (positions, indices) = arch();
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ONE);
    col.shape = ColliderShape::Mesh {
        convex: false,
        local_min: Vec3::ZERO,
        local_max: Vec3::ONE,
    };
    col.shared_mesh = Some(SharedMesh {
        positions,
        indices: indices.into(),
        ..Default::default()
    });
    scene.world.set_collider(id, Some(col));
    (scene, id)
}

fn make_dynamic(scene: &mut Scene, id: u32) {
    let rb = RigidBodyComponent {
        is_kinematic: false,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(rb));
}

#[test]
fn a_concave_mesh_on_a_dynamic_body_draws_its_pieces() {
    let (mut scene, id) = arched();
    let exact = collider_wire(&scene, id).expect("a trimesh").segments;
    make_dynamic(&mut scene, id);
    let pieces = collider_wire(&scene, id).expect("decomposed").segments;
    assert!(!pieces.is_empty());
    assert_ne!(exact, pieces, "the hulls, not the triangles");
}

#[test]
fn the_cache_builds_again_when_the_body_turns_dynamic() {
    let (mut scene, id) = arched();
    let mut cache = WireCache::default();
    cache.refresh(&scene);
    let exact = cache.wire(&scene, id).expect("a trimesh").segments.clone();
    make_dynamic(&mut scene, id);
    cache.refresh(&scene);
    let pieces = cache.wire(&scene, id).expect("decomposed").segments.clone();
    assert_ne!(exact, pieces);
    assert_eq!(cache.builds(), 2);
}
