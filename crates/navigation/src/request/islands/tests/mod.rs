//! The island cap (#2087): a goal on a crate top nothing climbs costs a few
//! thousand expansions, a goal on the floor the full cap, and no search more.

mod carry;
mod quiet;

use glam::Vec3;

use super::super::super::astar::MAX_SEARCH_NODES;
use super::super::super::test_support::{add_box, add_floor, ground};
use super::super::super::{NavBounds, NavigationGraph, ALL_AREAS};
use super::super::NodePool;
use super::UNREACHABLE_SEARCH_NODES;
use rusty_scene::Scene;

/// A 40 × 40 floor at the 0.25 m default cell, and a 2 m crate no step climbs.
fn level() -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -1.0, 41.0, -1.0, 41.0);
    add_box(
        &mut scene,
        Vec3::new(30.0, 0.0, 30.0),
        Vec3::new(36.0, 2.0, 36.0),
    );
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 40.0, 0.0, 40.0));
    NavigationGraph::from_scene(&scene)
}

#[test]
fn a_goal_on_an_island_is_searched_briefly() {
    let g = level();
    let start = g.snap(Vec3::new(2.0, 0.0, 2.0)).expect("floor");
    let top = g.snap(Vec3::new(33.0, 2.0, 33.0)).expect("crate top");
    let floor = g.snap(Vec3::new(38.0, 0.0, 38.0)).expect("floor");
    assert!(
        g.spans.len() as u32 > UNREACHABLE_SEARCH_NODES,
        "the cap bites"
    );
    assert_eq!(g.search_cap(start, top), UNREACHABLE_SEARCH_NODES);
    assert_eq!(g.search_cap(start, floor), MAX_SEARCH_NODES);
    let (path, complete) = g.find_path_masked(start, top, ALL_AREAS);
    assert!(!complete, "partial");
    let end = g.span_world(*path.last().expect("a path"));
    assert!(
        end.y < 0.5 && end.x > 20.0,
        "but headed for the crate: {end}"
    );
}

#[test]
fn an_unreachable_goal_costs_at_most_the_node_cap() {
    // 301² open spans; the goal's area is outside the mask, so A* floods them all.
    let mut g = NavigationGraph::new(0.0, 300.0, 0.0, 300.0, 1.0);
    let goal = g.span_refs().last().expect("spans");
    g.spans[goal.index as usize].area = 5;
    let start = g.span_refs().next().expect("spans");
    let mut pool = NodePool::default();
    pool.begin(g.spans.len());
    let mut search = g.begin_search(start, goal, ALL_AREAS & !(1 << 5), &mut pool);
    let mut budget = u32::MAX;
    let (path, complete) = g
        .step_search(&mut search, &mut pool, &mut budget)
        .expect("ends");
    assert!(!complete && path.len() > 1, "a partial path");
    assert!(search.capped(), "spent the whole cap");
    assert!(
        g.spans.len() as u32 > MAX_SEARCH_NODES,
        "the cap, not the mesh, ended it"
    );
}

/// A cell only a diagonal step reaches (#2087): its two cardinal cells stand
/// `max_step` up, it lies sunk below, its other neighbours are gone. A\* steps
/// into it, so the island map must not call it unreachable.
#[test]
fn a_cell_reached_only_diagonally_is_reachable() {
    let mut g = NavigationGraph::new(0.0, 4.0, 0.0, 4.0, 1.0);
    let s = g.max_step * 0.75;
    for (gx, gz, y) in [(2, 1, s), (1, 2, s), (2, 2, -s)] {
        let i = g.cell_start[g.index(gx, gz)] as usize;
        g.spans[i].y = y;
    }
    for (gx, gz) in [(3, 2), (2, 3)] {
        let hole = g.index(gx, gz);
        g.spans.remove(g.cell_start[hole] as usize);
        g.cell_start[hole + 1..].iter_mut().for_each(|s| *s -= 1);
    }
    let (start, sunk) = (g.snap(Vec3::ZERO).expect("floor"), ground(&g, 2, 2));
    assert!(g.neighbour(sunk, -1, 0).is_none() && g.neighbour(sunk, 0, -1).is_none());
    let (_, complete) = g.find_path_masked(start, sunk, ALL_AREAS);
    assert!(complete, "A* steps in diagonally");
    assert_eq!(g.search_cap(start, sunk), MAX_SEARCH_NODES);
}
