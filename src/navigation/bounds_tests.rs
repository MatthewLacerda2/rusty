//! Tests for the scene-derived navmesh bounds (`bounds.rs`, #452): the derived extent
//! and its margin, the authored override, the empty-scene default, spacing snapping,
//! and a path to geometry far past the old hardcoded ±20 box.

use super::bounds::resolve_bounds;
use super::test_support::add_box_with as add_box;
use super::*;
use crate::scene::Scene;
use glam::Vec3;

fn bounds(min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> NavBounds {
    NavBounds {
        min_x,
        max_x,
        min_z,
        max_z,
    }
}

#[test]
fn empty_scene_bakes_the_default_box() {
    let g = NavigationGraph::from_scene(&Scene::new());
    assert_eq!(g.bounds(), bounds(-20.0, 20.0, -20.0, 20.0));
    assert_eq!((g.width, g.height), (41, 41));
}

/// Derived = geometry extent grown by `BOUNDS_MARGIN + agent_radius` (2.0 + 0.5), snapped
/// outward to the unit lattice. Every side is asserted signed, so a flipped margin sign,
/// a swapped min/max fold or a floor↔ceil swap each fail.
#[test]
fn derived_bounds_are_geometry_plus_margin_snapped() {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        Vec3::new(1.2, 0.0, -3.7),
        Vec3::new(4.0, 1.0, 0.0),
        true,
    );
    add_box(
        &mut scene,
        Vec3::new(-6.0, 0.0, 2.0),
        Vec3::new(-5.0, 1.0, 5.1),
        true,
    );
    // x: [-6.0, 4.0] → [-8.5, 6.5] → [-9, 7]; z: [-3.7, 5.1] → [-6.2, 7.6] → [-7, 8].
    assert_eq!(resolve_bounds(&scene, 1.0), bounds(-9.0, 7.0, -7.0, 8.0));
}

#[test]
fn margin_grows_with_agent_radius() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::ZERO, Vec3::new(10.0, 1.0, 4.0), true);
    scene.nav_settings.agent_radius = 3.0;
    // Margin 2.0 + 3.0 = 5.0 on every side.
    assert_eq!(resolve_bounds(&scene, 1.0), bounds(-5.0, 15.0, -5.0, 9.0));
}

#[test]
fn snapping_follows_the_grid_spacing() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::ZERO, Vec3::new(10.0, 1.0, 10.0), true);
    // [-2.5, 12.5] snapped outward to multiples of 2.0.
    assert_eq!(resolve_bounds(&scene, 2.0), bounds(-4.0, 14.0, -4.0, 14.0));
}

#[test]
fn only_active_static_solid_colliders_count() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::ZERO, Vec3::ONE, true);
    add_box(&mut scene, Vec3::splat(90.0), Vec3::splat(95.0), false); // dynamic
    let off = add_box(&mut scene, Vec3::splat(-95.0), Vec3::splat(-90.0), true);
    scene.world.set_active(off, false);
    let trigger = add_box(&mut scene, Vec3::splat(50.0), Vec3::splat(55.0), true);
    scene
        .world
        .collider_mut(trigger)
        .expect("collider")
        .is_trigger = true;
    assert_eq!(resolve_bounds(&scene, 1.0), bounds(-3.0, 4.0, -3.0, 4.0));
}

#[test]
fn authored_override_wins_and_invalid_falls_back() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::ZERO, Vec3::ONE, true);
    scene.nav_settings.bounds = Some(bounds(-100.5, 30.0, 0.0, 12.0));
    assert_eq!(resolve_bounds(&scene, 1.0), bounds(-101.0, 30.0, 0.0, 12.0));
    // min ≥ max is ignored: the bake derives from the geometry instead.
    scene.nav_settings.bounds = Some(bounds(5.0, 5.0, 0.0, 12.0));
    assert_eq!(resolve_bounds(&scene, 1.0), bounds(-3.0, 4.0, -3.0, 4.0));
    scene.nav_settings.bounds = Some(bounds(0.0, f32::NAN, 0.0, 12.0));
    assert_eq!(resolve_bounds(&scene, 1.0), bounds(-3.0, 4.0, -3.0, 4.0));
}

#[test]
fn is_valid_needs_finite_positive_extent_on_both_axes() {
    assert!(bounds(0.0, 1.0, 0.0, 1.0).is_valid());
    assert!(!bounds(1.0, 0.0, 0.0, 1.0).is_valid());
    assert!(!bounds(0.0, 1.0, 1.0, 1.0).is_valid());
    assert!(!bounds(f32::NEG_INFINITY, 1.0, 0.0, 1.0).is_valid());
}

/// The bug: past x = 20 there was no grid, so an agent there never got a path. With a
/// floor at the origin and a platform at x = 60, the grid spans both and the path
/// steps toward the platform all the way there.
#[test]
fn geometry_at_x60_is_reachable_from_the_origin() {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        Vec3::new(-5.0, -0.1, -5.0),
        Vec3::new(65.0, 0.0, 5.0),
        true,
    );
    let g = NavigationGraph::from_scene(&scene);
    assert_eq!(g.bounds(), bounds(-8.0, 68.0, -8.0, 8.0));
    let target = Vec3::new(60.0, 0.0, 0.0);
    let mut pos = Vec3::ZERO;
    for _ in 0..200 {
        let next = g.get_next_path_step(pos, target);
        // One cell per step: a path clamped to a too-small grid would end with a jump
        // from its edge straight to the (unreachable) target.
        assert!(
            next.distance(pos) <= 1.5,
            "step {pos} -> {next} skips cells"
        );
        pos = next;
    }
    assert_eq!(pos, target, "walked all the way to x = 60");
}

/// Re-baking after the geometry grows re-shapes the grid (same dimensions formula as
/// `new`) and keeps the generation counter monotonic, so cached agent paths re-plan.
#[test]
fn rebake_reshapes_to_new_geometry() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::ZERO, Vec3::ONE, true);
    let mut g = NavigationGraph::from_scene(&scene);
    assert_eq!(g.bake_generation, 1);
    add_box(
        &mut scene,
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(41.0, 1.0, 1.0),
        true,
    );
    g.bake(&scene);
    assert_eq!(g.bake_generation, 2);
    assert_eq!(g.bounds(), bounds(-3.0, 44.0, -3.0, 4.0));
    let fresh = NavigationGraph::new(-3.0, 44.0, -3.0, 4.0, 1.0);
    assert_eq!((g.width, g.height), (fresh.width, fresh.height));
    assert_eq!(g.cell_start.len(), (48 * 8 + 1) as usize);
}

/// A zero, negative or infinite `grid_spacing` keeps the graph's current spacing, so the
/// bake never divides by zero or sizes an unbounded grid.
#[test]
fn invalid_spacing_keeps_the_current_one() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::ZERO, Vec3::ONE, true);
    for bad in [0.0, -1.0, f32::INFINITY, f32::NAN] {
        scene.nav_settings.grid_spacing = bad;
        let g = NavigationGraph::from_scene(&scene);
        assert_eq!(g.grid_spacing, 1.0, "spacing {bad} ignored");
        assert_eq!((g.width, g.height), (8, 8));
    }
}
