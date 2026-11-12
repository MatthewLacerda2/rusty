//! The island map across a rebake (#2111): carried, never rebuilt, under a moving
//! carving obstacle and an area-cost change, and never capping a goal a fresh map
//! would not.

use glam::Vec3;

use super::super::super::super::astar::MAX_SEARCH_NODES;
use super::super::super::super::test_support::{add_box, add_floor, add_obstacle, ground, move_to};
use super::super::super::super::{tick_obstacles, NavBounds, NavigationGraph, Rebake, SpanRef};
use super::super::UNREACHABLE_SEARCH_NODES;
use crate::scene::nav_settings::default_areas;
use crate::scene::Scene;

/// A 20 × 10 floor split by a wall at x = 10 with a doorway over z 3..8, a crate
/// no step climbs, a ledge the drop links leave, and a door-sized carving
/// obstacle standing at `door`.
fn level(door: Vec3) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 1.0;
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 10.0));
    scene.nav_settings.agent_radius = 0.4;
    scene.nav_settings.drop_height = 2.0;
    add_floor(&mut scene, -1.0, 21.0, -1.0, 11.0);
    let wall = |z0, z1| (Vec3::new(9.8, 0.0, z0), Vec3::new(10.2, 3.0, z1));
    for (min, max) in [wall(-1.0, 3.0), wall(8.0, 11.0)] {
        add_box(&mut scene, min, max);
    }
    add_box(
        &mut scene,
        Vec3::new(2.0, 0.0, 6.0),
        Vec3::new(5.0, 2.0, 9.0),
    );
    add_box(
        &mut scene,
        Vec3::new(14.0, 0.0, 6.0),
        Vec3::new(18.0, 1.2, 9.0),
    );
    let size = Vec3::new(0.4, 2.0, 5.0);
    let id = add_obstacle(&mut scene, door, size, true);
    (scene, id)
}

const SHUT: Vec3 = Vec3::new(10.0, 1.0, 5.5);
const OPEN: Vec3 = Vec3::new(1.0, 1.0, 5.5);

fn moved(scene: &mut Scene, g: &mut NavigationGraph, id: u32, at: Vec3) {
    move_to(scene, id, at);
    tick_obstacles(scene, 1.0 / 60.0);
    assert_ne!(g.sync(scene), Rebake::Full, "an incremental rebake");
}

#[test]
fn a_goal_the_obstacle_opens_is_not_capped() {
    let (mut scene, id) = level(SHUT);
    let mut g = NavigationGraph::from_scene(&scene);
    let (left, right) = (ground(&g, 3, 2), ground(&g, 17, 2));
    assert_eq!(g.search_cap(left, right), UNREACHABLE_SEARCH_NODES, "shut");
    moved(&mut scene, &mut g, id, OPEN);
    assert_eq!(g.search_cap(left, right), MAX_SEARCH_NODES, "opened");
    assert_eq!(g.islands.builds.get(), 1, "carried, not rebuilt");
}

#[test]
fn a_moving_obstacle_and_a_cost_change_never_rebuild_the_map() {
    let (mut scene, id) = level(OPEN);
    let mut g = NavigationGraph::from_scene(&scene);
    let (from, to) = (ground(&g, 1, 1), ground(&g, 18, 2));
    for t in 0..90 {
        if t % 3 == 0 {
            let at = OPEN + Vec3::new((t / 3 % 8) as f32 * 0.5, 0.0, 0.0);
            moved(&mut scene, &mut g, id, at);
        }
        assert_eq!(g.search_cap(from, to), MAX_SEARCH_NODES);
    }
    let mut areas = default_areas();
    areas[0].cost = 3.0;
    assert!(g.set_area_costs(&areas));
    assert_eq!(g.search_cap(from, to), MAX_SEARCH_NODES);
    assert_eq!(g.islands.builds.get(), 1, "built once, then carried");
}

/// After every rebake of an obstacle walking through the doorway and back, the
/// carried map caps no goal a fresh one would reach: it may merge more, never less.
#[test]
fn a_carried_map_caps_no_goal_a_fresh_one_reaches() {
    let (mut scene, id) = level(OPEN);
    let mut g = NavigationGraph::from_scene(&scene);
    let mut path = (0..12)
        .map(|i| OPEN.lerp(SHUT, i as f32 / 11.0))
        .collect::<Vec<_>>();
    path.extend(path.clone().into_iter().rev());
    for at in path {
        moved(&mut scene, &mut g, id, at);
        let carried = reach_sets(&g);
        let kept = g.islands.map.borrow_mut().take();
        let fresh = reach_sets(&g);
        *g.islands.map.borrow_mut() = kept;
        for (start, (c, f)) in carried.iter().zip(&fresh).enumerate() {
            let under = f.iter().find(|goal| !c.contains(goal));
            assert!(under.is_none(), "at {at}: {start} → {under:?} capped");
        }
    }
}

/// From each span, the spans the cached map says it may reach.
fn reach_sets(g: &NavigationGraph) -> Vec<Vec<SpanRef>> {
    let all: Vec<SpanRef> = g.span_refs().collect();
    let reach = |&start: &SpanRef| {
        let mut goals = all.clone();
        g.retain_reachable(start, &mut goals);
        goals
    };
    all.iter().map(reach).collect()
}
