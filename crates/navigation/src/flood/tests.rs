//! The bounded flood (#1892): it reaches what A\* reaches at the cost A\* walks,
//! stops at its bound, and never crosses a wall.

use glam::Vec3;

use crate::test_support::{add_box, add_floor, bake_pinned};
use crate::{Flood, NavigationGraph, ALL_AREAS};
use rusty_scene::Scene;

fn open_floor() -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    bake_pinned(&mut scene)
}

fn flood(nav: &NavigationGraph, from: Vec3, bound: f32) -> Flood<'_> {
    nav.flood(from, bound, ALL_AREAS)
        .expect("start on the navmesh")
}

#[test]
fn a_straight_run_costs_its_length() {
    let nav = open_floor();
    let f = flood(&nav, Vec3::new(2.5, 0.0, 2.5), 100.0);
    let len = f
        .length_to(&nav, Vec3::new(8.5, 0.0, 2.5))
        .expect("reached");
    assert!((len - 6.0).abs() < 1e-4, "{len}");
    let diag = f
        .length_to(&nav, Vec3::new(5.5, 0.0, 5.5))
        .expect("reached");
    assert!(
        (diag - 3.0 * std::f32::consts::SQRT_2).abs() < 1e-4,
        "{diag}"
    );
    assert_eq!(f.length_to(&nav, Vec3::new(2.5, 0.0, 2.5)), Some(0.0));
}

#[test]
fn the_bound_stops_the_flood() {
    let nav = open_floor();
    let near = flood(&nav, Vec3::new(2.5, 0.0, 2.5), 3.0);
    assert!(near.length_to(&nav, Vec3::new(5.5, 0.0, 2.5)).is_some());
    assert_eq!(near.length_to(&nav, Vec3::new(8.5, 0.0, 2.5)), None);
    let all = flood(&nav, Vec3::new(2.5, 0.0, 2.5), 100.0);
    assert!(near.reached() < all.reached(), "a bound covers less");
}

#[test]
fn a_wall_the_flood_cannot_cross_leaves_the_far_side_unreached() {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    // A wall right across the grid at x ≈ 5.
    add_box(
        &mut scene,
        Vec3::new(4.8, 0.0, -5.0),
        Vec3::new(5.2, 3.0, 15.0),
    );
    let nav = bake_pinned(&mut scene);
    let f = flood(&nav, Vec3::new(1.5, 0.0, 2.5), 100.0);
    assert!(f.length_to(&nav, Vec3::new(2.5, 0.0, 8.5)).is_some());
    assert_eq!(f.length_to(&nav, Vec3::new(8.5, 0.0, 2.5)), None);
}

#[test]
fn off_the_navmesh_is_no_flood() {
    // No floor at all: no span to start from.
    let nav = bake_pinned(&mut Scene::new());
    assert!(nav
        .flood(Vec3::new(2.5, 0.0, 2.5), 10.0, ALL_AREAS)
        .is_none());
}

#[test]
fn a_flood_to_goals_stops_once_they_settle_with_the_same_costs() {
    let nav = open_floor();
    let from = Vec3::new(2.5, 0.0, 2.5);
    let goal = Vec3::new(4.5, 0.0, 2.5);
    let full = flood(&nav, from, 100.0);
    let to = nav
        .flood_to(from, &[goal], 100.0, ALL_AREAS)
        .expect("on the navmesh");
    assert_eq!(to.length_to(&nav, goal), full.length_to(&nav, goal));
    assert!(
        to.reached() < full.reached(),
        "{} vs {}",
        to.reached(),
        full.reached()
    );
    let far = Vec3::new(500.0, 0.0, 500.0); // clamps onto the grid's corner
    let both = nav
        .flood_to(from, &[goal, far], 100.0, ALL_AREAS)
        .expect("flood");
    assert_eq!(both.length_to(&nav, goal), full.length_to(&nav, goal));
}
