use glam::{Quat, Vec3};

use crate::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_scene::Scene;

fn add_box(scene: &mut Scene, name: &str, pos: Vec3, size: Vec3) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_static(id, true);
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            shape: ColliderShape::Box { size },
            ..rusty_scene::create::default_collider()
        }),
    );
    id
}

fn two_box_world() -> (Scene, PhysicsWorld, u32, u32) {
    let mut scene = Scene::new();
    let near = add_box(&mut scene, "Near", Vec3::ZERO, Vec3::splat(2.0));
    let far = add_box(
        &mut scene,
        "Far",
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::splat(2.0),
    );
    let world = PhysicsWorld::from_scene(&scene);
    (scene, world, near, far)
}

#[test]
fn overlap_sphere_reports_only_colliders_inside() {
    let (_, world, near, far) = two_box_world();
    // Radius 2 at origin reaches `near` only; radius 12 swallows both (sorted).
    assert_eq!(world.overlap_sphere(Vec3::ZERO, 2.0, |_| true), vec![near]);
    let both = world.overlap_sphere(Vec3::ZERO, 12.0, |_| true);
    assert_eq!(both, {
        let mut v = vec![near, far];
        v.sort_unstable();
        v
    });
    assert!(world
        .overlap_sphere(Vec3::ZERO, 2.0, |id| id != near)
        .is_empty());
}

#[test]
fn overlap_box_and_capsule_respect_their_extents() {
    let (_, world, near, far) = two_box_world();
    assert!(world
        .overlap_box(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::splat(0.5),
            Quat::IDENTITY,
            |_| true
        )
        .is_empty());
    let hits = world.overlap_box(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(0.5, 0.5, 5.0),
        Quat::IDENTITY,
        |_| true,
    );
    assert_eq!(hits.len(), 2, "stretched box should reach both: {hits:?}");
    let capsule = world.overlap_capsule(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0), 0.5, |_| true);
    assert_eq!(capsule, hits, "capsule along the same span agrees");
    let _ = (near, far);
}

#[test]
fn sphere_cast_hits_what_a_thin_ray_misses() {
    let mut scene = Scene::new();
    // Box centered 1.5 up: a ray along +Z at y=0 passes under it (box bottom at
    // y=0.5), but a swept sphere of radius 1 clips it.
    let target = add_box(
        &mut scene,
        "High",
        Vec3::new(0.0, 1.5, 5.0),
        Vec3::splat(2.0),
    );
    let world = PhysicsWorld::from_scene(&scene);
    assert_eq!(
        world.raycast_hit(Vec3::ZERO, Vec3::Z, f32::MAX, |_| true),
        None
    );
    let hit = world
        .sphere_cast_hit(Vec3::ZERO, Vec3::Z, 1.0, f32::MAX, |_| true)
        .expect("swept sphere should clip the high box");
    let (id, toi) = (hit.id, hit.distance);
    assert_eq!(id, target);
    assert!(
        toi > 0.0 && toi < 5.0,
        "impact before the box center: {toi}"
    );
    assert_eq!(
        world.sphere_cast_hit(Vec3::ZERO, Vec3::Z, 1.0, f32::MAX, |id| id != target),
        None
    );
}

#[test]
fn closest_point_projects_onto_the_collider_surface() {
    let (_, world, near, _) = two_box_world();
    // `near` spans [-1,1]³; from (5,0,0) the closest point is the +X face.
    let p = world
        .closest_point_on(near, Vec3::new(5.0, 0.0, 0.0))
        .expect("near has a live collider");
    assert!((p - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-4, "got {p}");
    let inside = Vec3::new(0.25, 0.25, 0.25);
    let q = world.closest_point_on(near, inside).expect("live collider");
    assert!((q - inside).length() < 1e-4, "inside point echoes: {q}");
    assert_eq!(world.closest_point_on(9999, Vec3::ZERO), None);
}

#[test]
fn contains_point_answers_inside_outside_and_missing() {
    let (mut scene, world, near, far) = two_box_world();
    assert_eq!(world.collider_contains_point(near, Vec3::ZERO), Some(true));
    assert_eq!(
        world.collider_contains_point(near, Vec3::new(0.0, 0.0, 3.0)),
        Some(false)
    );
    assert_eq!(
        world.collider_contains_point(far, Vec3::new(0.0, 0.0, 10.5)),
        Some(true)
    );
    let bare = scene.add_entity("Bare".to_string());
    let world = PhysicsWorld::from_scene(&scene);
    assert_eq!(world.collider_contains_point(bare, Vec3::ZERO), None);
}
