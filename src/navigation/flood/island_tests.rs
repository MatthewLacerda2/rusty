//! The island check behind [`NavigationGraph::flood_to`] (#1892): a goal on a
//! small piece nothing walks into is dropped early, a one-way drop off it does
//! not count as a way in, and a big region's "not an island" answer is cached.

use glam::Vec3;

use super::islands::Islands;
use crate::navigation::test_support::{add_box, add_floor};
use crate::navigation::{NavBounds, NavigationGraph, ALL_AREAS};
use crate::scene::Scene;

/// A 60 × 60 m floor round the origin and a 2 m crate at x 3..5, z -1..1;
/// `drop_height` lets the bake add drops off the crate.
fn crate_level(drop_height: f32) -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -30.0, 30.0, -30.0, 30.0);
    add_box(
        &mut scene,
        Vec3::new(3.0, 0.0, -1.0),
        Vec3::new(5.0, 2.0, 1.0),
    );
    scene.nav_settings.drop_height = drop_height;
    scene.nav_settings.bounds = Some(NavBounds::new(-30.0, 30.0, -30.0, 30.0));
    let mut nav = NavigationGraph::new(-30.0, 30.0, -30.0, 30.0, 1.0);
    nav.bake(&scene);
    nav
}

/// The crate top is dropped without the flood running to its bound.
fn assert_top_dropped_early(nav: &NavigationGraph) {
    let (from, near, top) = (
        Vec3::ZERO,
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(4.0, 2.0, 0.0),
    );
    assert!(nav.snap(top).is_some(), "the crate top is navmesh");
    let full = nav.flood(from, 100.0, ALL_AREAS).expect("flood");
    let to = nav
        .flood_to(from, &[near, top], 100.0, ALL_AREAS)
        .expect("on the navmesh");
    assert_eq!(to.length_to(nav, near), full.length_to(nav, near));
    assert_eq!(to.length_to(nav, top), None, "unreachable");
    assert!(
        to.reached() * 10 < full.reached(),
        "the island does not hold the flood open: {} vs {}",
        to.reached(),
        full.reached()
    );
}

#[test]
fn a_goal_on_an_island_is_dropped_without_flooding_to_the_bound() {
    assert_top_dropped_early(&crate_level(0.0));
}

#[test]
fn a_drop_off_the_island_is_no_way_onto_it() {
    let nav = crate_level(4.0);
    assert!(nav.offmesh_links().count() > 0, "the bake made drops");
    assert!(
        nav.offmesh_links().all(|l| !l.bidirectional),
        "drops are one-way"
    );
    assert_top_dropped_early(&nav);
}

#[test]
fn a_big_region_is_answered_once() {
    let mut scene = Scene::new();
    add_floor(&mut scene, -30.0, 30.0, -30.0, 30.0);
    // A wall right across at x ≈ 0: two 30 × 60 m halves, each past the cap.
    add_box(
        &mut scene,
        Vec3::new(-0.3, 0.0, -30.0),
        Vec3::new(0.3, 3.0, 30.0),
    );
    scene.nav_settings.bounds = Some(NavBounds::new(-30.0, 30.0, -30.0, 30.0));
    let mut nav = NavigationGraph::new(-30.0, 30.0, -30.0, 30.0, 1.0);
    nav.bake(&scene);
    let snap = |x: f32, z: f32| nav.snap(Vec3::new(x, 0.0, z)).expect("navmesh");
    let (start, goal, other) = (snap(-10.0, 0.0), snap(10.0, 0.0), snap(11.0, 0.0));
    let mut islands = Islands::new(&nav);
    assert!(!islands.cut_off(goal, start), "too big to be an island");
    // The capped walk's region is kept, so a goal beside it is a lookup.
    assert!(islands.open.contains(&goal.index));
    assert!(islands.open.contains(&other.index), "the region is cached");
    assert!(!islands.cut_off(other, start));
    assert!(islands.cut.is_empty(), "nothing here is an island");
}
