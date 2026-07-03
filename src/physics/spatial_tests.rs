//! src/physics/spatial_tests.rs — the single test home for `spatial.rs` (#311).
//!
//! Asserts the *answers* the area queries compute (who is inside, how far the
//! swept sphere travels, where the closest point lands) against hand-computed
//! geometry, so a broken filter, a wrong shape placement, or a dropped hit is
//! caught — not merely that the calls return.

use std::f32::consts::FRAC_PI_4;

use glam::{Quat, Vec3};

use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// A static unit-ish box entity at `pos` with the given half-size box collider.
fn add_box(scene: &mut Scene, name: &str, pos: Vec3, size: Vec3) -> u32 {
    let id = scene.add_entity(name.to_string());
    if let Some(mut e) = scene.get_entity_mut(id) {
        e.transform.position = pos;
        e.is_static = true;
        e.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });
    }
    id
}

/// Two 2×2×2 boxes: `near` centered at origin, `far` at z=10.
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
    // The accept filter drops an otherwise-overlapping collider.
    assert!(world
        .overlap_sphere(Vec3::ZERO, 2.0, |id| id != near)
        .is_empty());
}

#[test]
fn overlap_box_and_capsule_respect_their_extents() {
    let (_, world, near, far) = two_box_world();
    // A thin box centered between the two entities touches neither…
    assert!(world
        .overlap_box(Vec3::new(0.0, 0.0, 5.0), Vec3::splat(0.5), |_| true)
        .is_empty());
    // …but stretched along z (half-extent 5 ⇒ spans z∈[0,10]) it touches both.
    let hits = world.overlap_box(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.5, 0.5, 5.0), |_| true);
    assert_eq!(hits.len(), 2, "stretched box should reach both: {hits:?}");
    // A capsule running from the origin to z=10 also spans both.
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
    assert_eq!(world.cast_ray(Vec3::ZERO, Vec3::Z, f32::MAX), None);
    let (id, toi) = world
        .cast_sphere_filtered(Vec3::ZERO, Vec3::Z, 1.0, f32::MAX, |_| true)
        .expect("swept sphere should clip the high box");
    assert_eq!(id, target);
    assert!(
        toi > 0.0 && toi < 5.0,
        "impact before the box center: {toi}"
    );
    // The accept filter makes the same sweep miss.
    assert_eq!(
        world.cast_sphere_filtered(Vec3::ZERO, Vec3::Z, 1.0, f32::MAX, |id| id != target),
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
    // Solid: a point inside is its own closest point.
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
    // An entity with no collider (or unknown id) has no live answer.
    let bare = scene.add_entity("Bare".to_string());
    let world = PhysicsWorld::from_scene(&scene);
    assert_eq!(world.collider_contains_point(bare, Vec3::ZERO), None);
}

#[test]
fn contains_point_follows_a_rotated_box_not_its_loose_aabb() {
    // A 2×2×2 box (half-extent 1) rotated 45° about Y: its corner now points
    // straight down the world X/Z axes, sqrt(2) ≈ 1.414 out from the center.
    // That diagonal reach is also exactly the box's loose AABB half-extent on
    // every axis (`he*cosθ + he*sinθ` at θ=45°), so a query point placed just
    // inside that AABB but off both corners is the case an AABB-only check
    // gets wrong: still "inside the bounds", but outside the actual diamond.
    let mut scene = Scene::new();
    let id = scene.add_entity("Diamond".to_string());
    if let Some(mut e) = scene.get_entity_mut(id) {
        e.transform.position = Vec3::ZERO;
        e.transform.rotation = Quat::from_rotation_y(FRAC_PI_4);
        e.is_static = true;
        e.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::splat(2.0),
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });
    }
    let world = PhysicsWorld::from_scene(&scene);

    // On the X axis alone, the rotated corner reaches to sqrt(2): inside only
    // because the rotation was applied (an unrotated half-extent-1 box would
    // reject this point outright).
    assert_eq!(
        world.collider_contains_point(id, Vec3::new(1.3, 0.0, 0.0)),
        Some(true)
    );
    // Off-axis and near the corner: within the box's loose AABB (which also
    // reaches sqrt(2) on x *and* z) but outside the actual rotated diamond.
    assert_eq!(
        world.collider_contains_point(id, Vec3::new(1.3, 0.0, 1.3)),
        Some(false)
    );
}

#[test]
fn contains_point_follows_a_sphere_not_its_bounding_cube() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Ball".to_string());
    if let Some(mut e) = scene.get_entity_mut(id) {
        e.is_static = true;
        e.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Sphere { radius: 1.0 },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });
    }
    let world = PhysicsWorld::from_scene(&scene);

    assert_eq!(
        world.collider_contains_point(id, Vec3::new(0.5, 0.5, 0.0)),
        Some(true)
    );
    // A corner of the sphere's bounding cube (±1 on every axis) sits well
    // outside the radius-1 ball itself — an AABB check would wrongly say yes.
    assert_eq!(
        world.collider_contains_point(id, Vec3::new(1.0, 1.0, 1.0)),
        Some(false)
    );
}
