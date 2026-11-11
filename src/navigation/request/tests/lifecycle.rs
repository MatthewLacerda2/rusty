//! The path queue across a rebake, and run to run (#2087).

use glam::Vec3;

use super::super::super::test_support::{add_obstacle, move_to};
use super::super::super::{tick_obstacles, NavigationGraph, Rebake};
use super::{agent, level, level_scene, spawn, sync_path, DT};
use crate::scene::Scene;

#[test]
fn a_rebake_mid_search_restarts_it() {
    let mut g = level();
    let mut scene = Scene::new();
    let (from, to) = (Vec3::new(15.0, 0.0, 2.0), Vec3::new(15.0, 0.0, 28.0));
    let id = spawn(&mut scene, from, to);
    g.set_pathfinding_iterations_per_frame(5);
    g.tick_nav_agents(&mut scene, DT);
    g.bake_generation += 1; // what a rebake does to a search in flight
    for _ in 0..1000 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let a = agent(&scene, id);
    assert_eq!(
        a.path_generation, g.bake_generation,
        "planned on the new bake"
    );
    assert!(
        scene.world.world_position(id).distance(to) < 1.0,
        "and arrived"
    );
}

/// A carving obstacle 30 m off rebakes every third tick while a long search is at
/// the head: the search is carried across each rebake (renumbered, to the path a
/// search on the final mesh finds), and every agent behind it gets its path.
#[test]
fn rebakes_far_from_a_search_do_not_stall_the_queue() {
    let mut geo = level_scene(60.0);
    let (size, at) = (Vec3::ONE, |t: usize| {
        Vec3::new(55.0, 0.5, 14.0 + (t / 3 % 2) as f32)
    });
    let obstacle = add_obstacle(&mut geo, at(0), size, true);
    let mut g = NavigationGraph::from_scene(&geo);
    let mut scene = Scene::new();
    let (from, to) = (Vec3::new(15.0, 0.0, 2.0), Vec3::new(15.0, 0.0, 28.0));
    let ids = [
        spawn(&mut scene, from, to),
        spawn(
            &mut scene,
            Vec3::new(2.0, 0.0, 2.0),
            Vec3::new(28.0, 0.0, 28.0),
        ),
        spawn(
            &mut scene,
            Vec3::new(28.0, 0.0, 2.0),
            Vec3::new(2.0, 0.0, 28.0),
        ),
    ];
    g.set_pathfinding_iterations_per_frame(32);
    let mut rebakes = 0;
    for t in 0..900 {
        if t % 3 == 0 {
            move_to(&mut geo, obstacle, at(t));
            tick_obstacles(&mut geo, DT);
            rebakes += matches!(g.sync(&geo), Rebake::Incremental(_)) as u32;
        }
        g.tick_nav_agents(&mut scene, DT);
    }
    assert!(rebakes > 200, "the obstacle kept rebaking: {rebakes}");
    for id in ids {
        let a = agent(&scene, id);
        assert!(
            a.path_planned && !a.cached_path.is_empty(),
            "agent {id} got a path"
        );
    }
    assert_eq!(agent(&scene, ids[0]).cached_path, sync_path(&g, from, to));
}

#[test]
fn queued_agents_are_deterministic() {
    let run = |budget: u32| {
        let g = level();
        let mut scene = Scene::new();
        g.set_pathfinding_iterations_per_frame(budget);
        let ids: Vec<u32> = (0..6)
            .map(|i| {
                let x = 2.0 + 5.0 * i as f32;
                spawn(
                    &mut scene,
                    Vec3::new(x, 0.0, 2.0),
                    Vec3::new(30.0 - x, 0.0, 28.0),
                )
            })
            .collect();
        for _ in 0..240 {
            g.tick_nav_agents(&mut scene, DT);
        }
        let at = |id: u32| scene.world.world_position(id).to_array();
        ids.into_iter().map(at).collect::<Vec<_>>()
    };
    assert_eq!(run(16), run(16), "same scene, same budget, same positions");
}
