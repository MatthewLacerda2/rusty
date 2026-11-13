//! The flood stepped on a budget (#2116): sliced as finely as one expansion a
//! step, it settles every goal at the very cost the synchronous flood gives it,
//! and a full rebake between two steps starts it over on the new navmesh (an
//! incremental one that misses it carries it: `ai::query::carry_tests`).

use glam::Vec3;

use crate::test_support::{add_box, add_floor, move_to};
use crate::{FloodSearch, NavBounds, NavigationGraph, ALL_AREAS};
use rusty_scene::Scene;

/// A 40 × 40 m floor, a wall with a doorway, and a box that can move.
fn level() -> (Scene, u32) {
    let mut scene = Scene::new();
    add_floor(&mut scene, -20.0, 20.0, -20.0, 20.0);
    add_box(
        &mut scene,
        Vec3::new(-0.3, 0.0, -20.0),
        Vec3::new(0.3, 3.0, -1.0),
    );
    add_box(
        &mut scene,
        Vec3::new(-0.3, 0.0, 1.0),
        Vec3::new(0.3, 3.0, 20.0),
    );
    let block = add_box(
        &mut scene,
        Vec3::new(4.0, 0.0, -3.0),
        Vec3::new(5.0, 3.0, 3.0),
    );
    scene.nav_settings.bounds = Some(NavBounds::new(-20.0, 20.0, -20.0, 20.0));
    (scene, block)
}

fn bake(scene: &Scene) -> NavigationGraph {
    let mut nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
    nav.bake(scene);
    nav
}

const FROM: Vec3 = Vec3::new(-8.0, 0.0, 0.0);
const BOUND: f32 = 60.0;

fn goals() -> Vec<Vec3> {
    let floor = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    vec![
        floor(8.0, 0.0),
        floor(-8.0, 6.0),
        floor(12.0, -10.0),
        floor(3.0, 15.0),
    ]
}

/// Each goal's cost from the synchronous flood, as `f32` bits.
fn synchronous(nav: &NavigationGraph, goals: &[Vec3]) -> Vec<Option<u32>> {
    let flood = nav
        .flood_to(FROM, goals, BOUND, ALL_AREAS)
        .expect("on the navmesh");
    goals
        .iter()
        .map(|&g| flood.length_to(nav, g).map(f32::to_bits))
        .collect()
}

/// Step `search` `per_step` expansions at a time to its end: how many steps.
fn run(nav: &NavigationGraph, search: &mut FloodSearch, per_step: u32) -> usize {
    let mut steps = 0;
    loop {
        steps += 1;
        let mut budget = per_step;
        if nav.step_flood(search, &mut budget) {
            return steps;
        }
        assert_eq!(
            budget, 0,
            "a step that stops short of the end spent its budget"
        );
    }
}

#[test]
fn a_sliced_flood_gives_each_goal_the_synchronous_cost_to_the_bit() {
    let (scene, _) = level();
    let nav = bake(&scene);
    let goals = goals();
    let want = synchronous(&nav, &goals);
    assert!(want.iter().all(Option::is_some), "every goal is reachable");
    for per_step in [1, 7, 64, u32::MAX] {
        let mut search = FloodSearch::default();
        nav.begin_flood(&mut search, FROM, &goals, BOUND, ALL_AREAS);
        let steps = run(&nav, &mut search, per_step);
        let got: Vec<_> = goals
            .iter()
            .map(|&g| search.length_to(&nav, g).map(f32::to_bits))
            .collect();
        assert_eq!(got, want, "{per_step} expansions a step");
        assert!(per_step > 64 || steps > 1, "a small budget slices it");
    }
}

#[test]
fn a_full_rebake_between_steps_starts_the_flood_over_on_the_new_navmesh() {
    let (mut scene, block) = level();
    let mut nav = bake(&scene);
    let goals = goals();
    let mut search = FloodSearch::default();
    nav.begin_flood(&mut search, FROM, &goals, BOUND, ALL_AREAS);
    let mut budget = 40;
    assert!(!nav.step_flood(&mut search, &mut budget), "unfinished");
    move_to(&mut scene, block, Vec3::new(-4.0, 0.0, 0.0)); // in the way now
    nav.bake(&scene);
    run(&nav, &mut search, 16);
    let got: Vec<_> = goals
        .iter()
        .map(|&g| search.length_to(&nav, g).map(f32::to_bits))
        .collect();
    assert_eq!(got, synchronous(&nav, &goals), "the new navmesh's costs");
    assert_ne!(
        got,
        synchronous(&bake(&level().0), &goals),
        "the block moved the costs"
    );
}

#[test]
fn a_start_off_the_navmesh_finishes_at_once_reaching_nothing() {
    let mut scene = Scene::new();
    add_floor(&mut scene, 10.0, 20.0, 10.0, 20.0); // a floor far from `FROM`
    let nav = bake(&scene);
    let mut search = FloodSearch::default();
    nav.begin_flood(&mut search, FROM, &goals(), BOUND, ALL_AREAS);
    let mut budget = 5;
    assert!(nav.step_flood(&mut search, &mut budget), "finished");
    assert_eq!(budget, 5, "nothing expanded");
    assert_eq!(search.length_to(&nav, Vec3::new(15.0, 0.0, 15.0)), None);
}
