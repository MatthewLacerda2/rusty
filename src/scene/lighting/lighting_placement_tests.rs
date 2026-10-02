//! Tests for deterministic IBL auto-placement (#246). Sibling of `placement.rs`.
//! Placement takes walkable bounds as plain `Vec3`s (#721); how the nav graph
//! computes them is tested in `navigation` (`walkable_tests.rs`).
//!
//! The bake itself needs a GPU, but PLACEMENT is pure math — so these run anywhere,
//! adapter or not. They pin two acceptance points: placement is a deterministic
//! function of the scene (same scene → same positions) and the counts/bounds are
//! sensible for a known box scene.

use glam::Vec3;

use super::{plan_light_probes, plan_reflection_probes, static_scene_aabb};
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// Add a static box entity with a box collider of `size`, centred at `pos`.
fn add_static_box(scene: &mut Scene, name: &str, pos: Vec3, size: Vec3) {
    let id = scene.add_entity(name.to_string());
    scene.world.set_static(id, true);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = pos;
    }
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size },
            is_trigger: false,
            material: Default::default(),
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    scene.update_entity_collider(id);
}

/// A 20×6×20 room: a single static floor box spanning [-10,10] in XZ, 6 tall.
fn room_scene() -> Scene {
    let mut scene = Scene::new();
    add_static_box(
        &mut scene,
        "Floor",
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(20.0, 6.0, 20.0),
    );
    scene
}

#[test]
fn aabb_of_known_box_is_sensible() {
    let scene = room_scene();
    let (min, max) = static_scene_aabb(&scene).expect("static box bounds");
    assert!((min - Vec3::new(-10.0, -3.0, -10.0)).length() < 1e-4);
    assert!((max - Vec3::new(10.0, 3.0, 10.0)).length() < 1e-4);
}

#[test]
fn aabb_none_when_no_static_collider() {
    let scene = Scene::new();
    assert!(static_scene_aabb(&scene).is_none());
}

#[test]
fn light_probe_plan_falls_back_to_scene_aabb_without_nav() {
    let scene = room_scene();
    let plan = plan_light_probes(&scene, None, 5.0).expect("a plan from the static AABB");
    // Falls back to the static box bounds when no baked nav surface is supplied.
    assert!((plan.min - Vec3::new(-10.0, -3.0, -10.0)).length() < 1e-4);
    assert!((plan.max - Vec3::new(10.0, 3.0, 10.0)).length() < 1e-4);
    assert_eq!(plan.spacing, 5.0);
}

#[test]
fn light_probe_spacing_is_clamped() {
    let scene = room_scene();
    let plan = plan_light_probes(&scene, None, 0.01).unwrap();
    assert!(plan.spacing >= 0.5, "tiny spacing must clamp up");
}

#[test]
fn light_probe_plan_none_on_empty_scene() {
    let scene = Scene::new();
    assert!(plan_light_probes(&scene, None, 4.0).is_none());
}

/// Walkable bounds narrower than the geometry box: the grid clips to the
/// intersection in XZ (the navigable rectangle) and spans the walkable floors up
/// through the actor headroom in Y.
#[test]
fn light_probe_plan_uses_walkable_bounds() {
    let scene = room_scene();
    let walkable = (Vec3::new(-4.0, 0.0, -4.0), Vec3::new(4.0, 0.0, 4.0));
    let plan = plan_light_probes(&scene, Some(walkable), 4.0).unwrap();
    assert_eq!(plan.min, Vec3::new(-4.0, 0.0, -4.0));
    assert_eq!(
        plan.max,
        Vec3::new(4.0, 3.0, 4.0),
        "floor + 3 units of headroom"
    );
}

/// Walkable bounds wider than the level are clipped to its static box in XZ; the
/// actor volume's Y range is kept even past the geometry.
#[test]
fn light_probe_plan_clips_walkable_bounds_to_the_level() {
    let scene = room_scene();
    let walkable = (Vec3::new(-30.0, 2.0, -5.0), Vec3::new(30.0, 2.5, 5.0));
    let plan = plan_light_probes(&scene, Some(walkable), 4.0).unwrap();
    assert_eq!(plan.min, Vec3::new(-10.0, 2.0, -5.0));
    assert_eq!(plan.max, Vec3::new(10.0, 5.5, 5.0));
}

/// Walkable bounds that miss the level entirely leave nothing to fill.
#[test]
fn light_probe_plan_none_when_walkable_misses_the_level() {
    let scene = room_scene();
    let walkable = (Vec3::new(50.0, 0.0, 50.0), Vec3::new(60.0, 0.0, 60.0));
    assert!(plan_light_probes(&scene, Some(walkable), 4.0).is_none());
}

/// Walkable bounds alone place a grid when the scene has no static geometry.
#[test]
fn light_probe_plan_from_walkable_bounds_alone() {
    let walkable = (Vec3::new(-2.0, 1.0, -2.0), Vec3::new(2.0, 1.0, 2.0));
    let plan = plan_light_probes(&Scene::new(), Some(walkable), 4.0).unwrap();
    assert_eq!((plan.min, plan.max), (walkable.0, Vec3::new(2.0, 4.0, 2.0)));
}

#[test]
fn reflection_plan_subdivides_into_regions() {
    let scene = room_scene();
    // 20-unit XZ span / 10-unit region ≈ 2 regions per horizontal axis; 6-unit Y → 1.
    let plans = plan_reflection_probes(&scene, 10.0, 4);
    // 2 (x) × 1 (y, thin box) × 2 (z) = 4 regions.
    assert_eq!(plans.len(), 4, "2×1×2 region grid over the box");
    // Each region's box is inside the scene AABB and its centroid is its box centre.
    let (amin, amax) = static_scene_aabb(&scene).unwrap();
    for p in &plans {
        assert!(p.box_min.cmpge(amin - Vec3::splat(1e-4)).all());
        assert!(p.box_max.cmple(amax + Vec3::splat(1e-4)).all());
        assert!(((p.box_min + p.box_max) * 0.5 - p.position).length() < 1e-4);
    }
}

#[test]
fn reflection_plan_empty_without_static_geometry() {
    let scene = Scene::new();
    assert!(plan_reflection_probes(&scene, 10.0, 4).is_empty());
}

#[test]
fn reflection_per_axis_cap_is_honoured() {
    let mut scene = Scene::new();
    add_static_box(
        &mut scene,
        "BigFloor",
        Vec3::ZERO,
        Vec3::new(1000.0, 4.0, 1000.0),
    );
    let plans = plan_reflection_probes(&scene, 5.0, 3);
    // Cap of 3 per horizontal axis, vertical capped to min(cap,2)→1 for the thin box:
    // 3 (x) × 1 (y) × 3 (z) = 9.
    assert_eq!(plans.len(), 9);
}

#[test]
fn placement_is_deterministic() {
    // Same scene built twice → identical plans (no RNG, no clock, stable iteration).
    let a = room_scene();
    let b = room_scene();
    let walkable = Some((Vec3::new(-8.0, 0.0, -8.0), Vec3::new(8.0, 0.0, 8.0)));
    assert_eq!(
        plan_light_probes(&a, walkable, 4.0),
        plan_light_probes(&b, walkable, 4.0)
    );
    assert_eq!(
        plan_reflection_probes(&a, 10.0, 4),
        plan_reflection_probes(&b, 10.0, 4)
    );
}
