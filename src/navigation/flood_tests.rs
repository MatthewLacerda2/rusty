//! The bounded flood (#1892): it reaches what A\* reaches at the cost A\* walks,
//! stops at its bound, and never crosses a wall.

use glam::Vec3;

use super::test_support::{add_box, add_floor, bake_pinned};
use super::{Flood, NavigationGraph, ALL_AREAS};
use crate::scene::Scene;

fn open_floor() -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    bake_pinned(&mut scene)
}

fn flood(nav: &NavigationGraph, from: Vec3, bound: f32) -> Flood {
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
    let nav = open_floor();
    assert!(nav
        .flood(Vec3::new(500.0, 0.0, 500.0), 10.0, ALL_AREAS)
        .is_none());
}
