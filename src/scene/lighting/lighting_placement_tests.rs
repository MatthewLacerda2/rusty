//! Tests for deterministic IBL auto-placement (#246). Sibling of
//! `lighting_placement.rs`; shares its 300-line source cap.
//!
//! The bake itself needs a GPU, but PLACEMENT is pure math — so these run anywhere,
//! adapter or not. They pin two acceptance points: placement is a deterministic
//! function of the scene (same scene → same positions) and the counts/bounds are
//! sensible for a known box scene.

use glam::Vec3;

use super::{plan_light_probes, plan_reflection_probes, static_scene_aabb};
use crate::components::{ColliderComponent, ColliderShape};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// Add a static box entity with a box collider of `size`, centred at `pos`.
fn add_static_box(scene: &mut Scene, name: &str, pos: Vec3, size: Vec3) {
    let id = scene.add_entity(name.to_string());
    let mut e = scene.get_entity_mut(id).unwrap();
    e.is_static = true;
    e.transform.position = pos;
    e.collider = Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    });
    drop(e);
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

#[test]
fn light_probe_plan_uses_nav_bounds_when_baked() {
    let scene = room_scene();
    // A baked nav surface narrower than the geometry box: the grid should clip to the
    // intersection in XZ (the navigable rectangle), not the wider geometry.
    let mut nav = NavigationGraph::new(-4.0, 4.0, -4.0, 4.0, 1.0);
    nav.bake_generation = 1;
    let plan = plan_light_probes(&scene, Some(&nav), 4.0).unwrap();
    assert!(plan.min.x >= -4.0 - 1e-4 && plan.max.x <= 4.0 + 1e-4);
    assert!(plan.min.z >= -4.0 - 1e-4 && plan.max.z <= 4.0 + 1e-4);
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

/// Probe-placement interaction with agent-radius erosion (#277). Light-probe bounds
/// follow the baked WALKABLE extent, so eroding the surface (a larger agent radius) must
/// TIGHTEN — never grow — the placement bounds: probes track where agents can actually
/// stand. Built on a flat scene so erosion pulls the walkable rim in off the world edge.
#[test]
fn erosion_tightens_light_probe_bounds() {
    let bake_with = |radius: f32| {
        let mut scene = room_scene();
        scene.nav_settings.agent_radius = radius;
        let mut nav = NavigationGraph::new(-10.0, 10.0, -10.0, 10.0, 1.0);
        nav.bake(&scene);
        plan_light_probes(&scene, Some(&nav), 4.0).expect("a plan from the baked nav")
    };
    let wide = bake_with(0.0); // no erosion: full walkable rectangle
    let eroded = bake_with(2.0); // erodes the outer rim by ~2 cells
    let wide_span = wide.max - wide.min;
    let eroded_span = eroded.max - eroded.min;
    assert!(
        eroded_span.x <= wide_span.x + 1e-4 && eroded_span.z <= wide_span.z + 1e-4,
        "erosion must not grow the probe bounds (x/z)"
    );
    assert!(
        eroded_span.x < wide_span.x - 1e-4 || eroded_span.z < wide_span.z - 1e-4,
        "a 2-unit radius visibly tightens the probe bounds off the world edge"
    );
}

#[test]
fn placement_is_deterministic() {
    // Same scene built twice → identical plans (no RNG, no clock, stable iteration).
    let a = room_scene();
    let b = room_scene();
    let mut nav = NavigationGraph::new(-8.0, 8.0, -8.0, 8.0, 1.0);
    nav.bake_generation = 1;
    assert_eq!(
        plan_light_probes(&a, Some(&nav), 4.0),
        plan_light_probes(&b, Some(&nav), 4.0)
    );
    assert_eq!(
        plan_reflection_probes(&a, 10.0, 4),
        plan_reflection_probes(&b, 10.0, 4)
    );
}
