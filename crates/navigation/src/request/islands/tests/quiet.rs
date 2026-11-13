//! A carried island map after the navmesh stands still (#2111): rebuilt fresh once,
//! so a split the carry merged is seen; never while an obstacle keeps moving.

use super::super::super::super::astar::MAX_SEARCH_NODES;
use super::super::super::super::test_support::ground;
use super::super::super::super::{NavigationGraph, Rebake};
use super::super::carry::QUIET_SYNCS;
use super::super::UNREACHABLE_SEARCH_NODES;
use super::carry::{level, moved, OPEN, SHUT};
use glam::Vec3;

/// A door that shuts and stays shut: the carry keeps the rooms one area, so the
/// search keeps the full cap until a quiet spell rebuilds the map, then is capped.
#[test]
fn a_door_that_stays_shut_caps_the_room_behind_it_after_a_quiet_spell() {
    let (mut scene, id) = level(OPEN);
    let mut g = NavigationGraph::from_scene(&scene);
    let (left, right) = (ground(&g, 3, 2), ground(&g, 17, 2));
    assert_eq!(g.search_cap(left, right), MAX_SEARCH_NODES, "open");
    moved(&mut scene, &mut g, id, SHUT);
    assert_eq!(
        g.search_cap(left, right),
        MAX_SEARCH_NODES,
        "carried: merged"
    );
    for tick in 1..QUIET_SYNCS {
        assert_eq!(g.sync(&scene), Rebake::Unchanged);
        assert_eq!(g.search_cap(left, right), MAX_SEARCH_NODES, "tick {tick}");
    }
    assert_eq!(g.sync(&scene), Rebake::Unchanged);
    assert_eq!(
        g.search_cap(left, right),
        UNREACHABLE_SEARCH_NODES,
        "rebuilt"
    );
    assert_eq!(
        g.islands.builds.get(),
        2,
        "built, carried, built fresh once"
    );
    for _ in 0..QUIET_SYNCS * 2 {
        g.sync(&scene);
        g.search_cap(left, right);
    }
    assert_eq!(g.islands.builds.get(), 2, "a fresh map is never rebuilt");
}

/// An obstacle that moves every 3 ticks, synced every tick: never a fresh build.
#[test]
fn an_obstacle_moving_every_three_ticks_never_rebuilds_the_map() {
    let (mut scene, id) = level(OPEN);
    let mut g = NavigationGraph::from_scene(&scene);
    let (from, to) = (ground(&g, 1, 1), ground(&g, 18, 2));
    for t in 0..QUIET_SYNCS * 4 {
        if t % 3 == 0 {
            let at = OPEN + Vec3::new((t / 3 % 8) as f32 * 0.5, 0.0, 0.0);
            moved(&mut scene, &mut g, id, at);
        } else {
            assert_eq!(g.sync(&scene), Rebake::Unchanged);
        }
        assert_eq!(g.search_cap(from, to), MAX_SEARCH_NODES);
    }
    assert_eq!(g.islands.builds.get(), 1, "built once, then carried");
}
