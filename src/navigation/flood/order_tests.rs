//! The flood's kept state (#2088): the search pool and the island answers it
//! carries between queries never change an answer. The same queries give the
//! same costs in any order, on a fresh graph or one that ran others first, and
//! a rebake drops what the old navmesh proved.

use glam::Vec3;

use crate::navigation::test_support::{add_box, add_floor, move_to};
use crate::navigation::{NavBounds, NavigationGraph, ALL_AREAS};
use crate::scene::Scene;

/// A 40 × 40 m floor, a wall with a doorway, a 2 m crate (an island, with drops
/// off it) and its box id, so a test can move it.
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
    let crate_box = add_box(
        &mut scene,
        Vec3::new(6.0, 0.0, 6.0),
        Vec3::new(8.0, 2.0, 8.0),
    );
    scene.nav_settings.drop_height = 4.0;
    scene.nav_settings.bounds = Some(NavBounds::new(-20.0, 20.0, -20.0, 20.0));
    (scene, crate_box)
}

fn bake(scene: &Scene) -> NavigationGraph {
    let mut nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
    nav.bake(scene);
    nav
}

const TOP: Vec3 = Vec3::new(7.0, 2.0, 7.0);

/// `(start, goals)`: across the doorway, onto and off the crate, from its top.
fn queries() -> Vec<(Vec3, Vec<Vec3>)> {
    let floor = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    vec![
        (
            floor(-8.0, 0.0),
            vec![floor(8.0, 0.0), TOP, floor(-8.0, 6.0)],
        ),
        (
            TOP,
            vec![Vec3::new(7.5, 2.0, 6.5), floor(10.0, 7.0), floor(-5.0, 0.0)],
        ),
        (
            floor(10.0, 10.0),
            vec![TOP, floor(5.0, 7.0), floor(-10.0, -10.0)],
        ),
        (
            floor(3.0, 0.0),
            vec![floor(-3.0, 0.0), TOP, floor(15.0, 15.0)],
        ),
    ]
}

/// Each goal's length, per query, run in the order given.
fn run(nav: &NavigationGraph, order: &[usize]) -> Vec<(usize, Vec<Option<f32>>)> {
    let all = queries();
    order
        .iter()
        .map(|&q| {
            let (start, goals) = &all[q];
            let f = nav
                .flood_to(*start, goals, 60.0, ALL_AREAS)
                .expect("on the navmesh");
            (q, goals.iter().map(|&g| f.length_to(nav, g)).collect())
        })
        .collect()
}

#[test]
fn the_same_queries_answer_the_same_in_any_order_and_on_a_fresh_graph() {
    let (scene, _) = level();
    let alone: Vec<_> = (0..queries().len())
        .map(|q| run(&bake(&scene), &[q]).remove(0))
        .collect();
    let nav = bake(&scene);
    let forward = run(&nav, &[0, 1, 2, 3]);
    let a = nav.snap(Vec3::new(-8.0, 0.0, 0.0)).expect("navmesh");
    let b = nav.snap(Vec3::new(8.0, 0.0, 0.0)).expect("navmesh");
    assert!(
        nav.find_path_masked(a, b, ALL_AREAS).1,
        "A* between shares the pool"
    );
    let backward = run(&nav, &[3, 2, 1, 0, 1, 3]);
    assert_eq!(forward, alone);
    let mut first_four = backward[..4].to_vec();
    first_four.sort_by_key(|r| r.0);
    assert_eq!(first_four, alone);
    assert_eq!(backward[4], alone[1], "a repeat after the others");
    assert_eq!(alone[0].1[1], None, "no way up the crate");
    assert!(
        alone[1].1.iter().all(Option::is_some),
        "the top drops to the floor"
    );
}

#[test]
fn a_flood_held_while_another_runs_keeps_its_costs() {
    let (scene, _) = level();
    let nav = bake(&scene);
    let goal = Vec3::new(8.0, 0.0, 0.0);
    let first = nav
        .flood(Vec3::new(-8.0, 0.0, 0.0), 60.0, ALL_AREAS)
        .expect("flood");
    let before = first.length_to(&nav, goal);
    let second = nav
        .flood(Vec3::new(10.0, 0.0, 10.0), 60.0, ALL_AREAS)
        .expect("flood");
    assert_ne!(second.length_to(&nav, goal), before, "a different start");
    assert_eq!(first.length_to(&nav, goal), before);
    drop(first);
    let third = nav
        .flood(Vec3::new(-8.0, 0.0, 0.0), 60.0, ALL_AREAS)
        .expect("flood");
    assert_eq!(third.length_to(&nav, goal), before);
}

#[test]
fn a_rebake_drops_the_islands_the_old_navmesh_proved() {
    let (mut scene, crate_box) = level();
    let mut nav = bake(&scene);
    // Behind the wall: the long way round makes the goal overdue (islands asked).
    let from = Vec3::new(-1.5, 0.0, 10.0);
    let on_crate = |nav: &NavigationGraph, top: Vec3| {
        let f = nav.flood_to(from, &[top], 60.0, ALL_AREAS).expect("flood");
        f.length_to(nav, top)
    };
    assert_eq!(on_crate(&nav, TOP), None, "an island");
    assert_eq!(nav.flood_cache.with(&nav, |c| c.islands.pieces.len()), 1);
    // Sink the crate to a 0.2 m step: its top is now walked onto.
    move_to(&mut scene, crate_box, Vec3::new(7.0, -0.8, 7.0));
    nav.bake(&scene);
    let low = Vec3::new(7.0, 0.2, 7.0);
    assert!(on_crate(&nav, low).is_some(), "reached");
    let pieces = nav.flood_cache.with(&nav, |c| c.islands.pieces.len());
    assert_eq!(pieces, 0, "the old crate top's piece is forgotten");
}
