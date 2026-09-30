//! src/physics/raycast_tests.rs — ray hit info and the all-hits cast (#446).
//!
//! Asserts the *signed* point and normal each face reports, and the exact order
//! `raycast_all` returns — so a flipped normal, a point off the surface, or an
//! order leaking parry's traversal is caught, not merely that a hit came back.

use glam::Vec3;

use super::query::{sort_hits, RayHit};
use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// A static box entity centered at `pos` with a `size` box collider.
fn add_box(scene: &mut Scene, pos: Vec3, size: Vec3) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_static(id, true);
    let shape = ColliderShape::Box { size };
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            active: true,
            shape,
            is_trigger: false,
            material: Default::default(),
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    id
}

/// `a` equals `b` component-wise within a small tolerance, sign included.
fn assert_near(a: Vec3, b: Vec3, what: &str) {
    assert!(
        (a - b).abs().max_element() < 1e-3,
        "{what}: got {a:?}, want {b:?}"
    );
}

#[test]
fn each_box_face_reports_its_signed_normal_and_surface_point() {
    let mut scene = Scene::new();
    let target = add_box(&mut scene, Vec3::ZERO, Vec3::splat(2.0));
    let world = PhysicsWorld::from_scene(&scene);
    for face in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
        // Fire from 5 m out along `face` back at the center: the face at 1 m.
        let hit = world
            .raycast_hit(face * 5.0, -face, 20.0, |_| true)
            .expect("hits the box");
        assert_eq!(hit.id, target);
        assert!(
            (hit.distance - 4.0).abs() < 1e-3,
            "{face:?}: {}",
            hit.distance
        );
        assert_near(hit.normal, face, "normal");
        assert_near(hit.point, face, "point");
    }
}

/// Three 1 m boxes along +Z at z = 9, 3, 6 — created far-first so id order
/// and distance order disagree.
fn stacked() -> (PhysicsWorld, [u32; 3]) {
    let mut scene = Scene::new();
    let far = add_box(&mut scene, Vec3::new(0.0, 0.0, 9.0), Vec3::ONE);
    let near = add_box(&mut scene, Vec3::new(0.0, 0.0, 3.0), Vec3::ONE);
    let mid = add_box(&mut scene, Vec3::new(0.0, 0.0, 6.0), Vec3::ONE);
    (PhysicsWorld::from_scene(&scene), [near, mid, far])
}

#[test]
fn raycast_all_returns_every_box_nearest_first_with_entry_info() {
    let (world, ids) = stacked();
    let hits = world.raycast_all(Vec3::ZERO, Vec3::Z, 20.0, |_| true);
    assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), ids);
    for (hit, entry) in hits.iter().zip([2.5, 5.5, 8.5]) {
        assert!((hit.distance - entry).abs() < 1e-3, "{hit:?}");
        assert_near(hit.point, Vec3::Z * entry, "entry point");
        assert_near(hit.normal, -Vec3::Z, "entry normal");
    }
}

#[test]
fn raycast_all_honours_max_distance_and_the_accept_filter() {
    let (world, [near, mid, far]) = stacked();
    let short = world.raycast_all(Vec3::ZERO, Vec3::Z, 6.0, |_| true);
    assert_eq!(short.iter().map(|h| h.id).collect::<Vec<_>>(), [near, mid]);
    let skip_mid = world.raycast_all(Vec3::ZERO, Vec3::Z, 20.0, |id| id != mid);
    assert_eq!(
        skip_mid.iter().map(|h| h.id).collect::<Vec<_>>(),
        [near, far]
    );
    assert!(world
        .raycast_all(Vec3::ZERO, -Vec3::Z, 20.0, |_| true)
        .is_empty());
}

#[test]
fn equal_distances_are_ordered_by_entity_id() {
    let hit = |id, distance| RayHit {
        id,
        distance,
        point: Vec3::ZERO,
        normal: Vec3::Z,
    };
    let mut hits = [hit(7, 2.0), hit(3, 2.0), hit(5, 1.0), hit(1, 3.0)];
    sort_hits(&mut hits);
    let order: Vec<_> = hits.iter().map(|h| h.id).collect();
    assert_eq!(order, [5, 3, 7, 1]);
}

#[test]
fn sphere_cast_reports_the_contact_point_and_face_normal() {
    let mut scene = Scene::new();
    let target = add_box(&mut scene, Vec3::new(0.0, 0.0, 5.0), Vec3::splat(2.0));
    let world = PhysicsWorld::from_scene(&scene);
    // A 0.5 m sphere meets the z=4 face after its center travels 3.5 m.
    let hit = world
        .sphere_cast_hit(Vec3::ZERO, Vec3::Z, 0.5, 20.0, |_| true)
        .expect("hits the box");
    assert_eq!(hit.id, target);
    assert!((hit.distance - 3.5).abs() < 1e-3, "{}", hit.distance);
    assert_near(hit.point, Vec3::new(0.0, 0.0, 4.0), "contact point");
    assert_near(hit.normal, -Vec3::Z, "face normal");
}
