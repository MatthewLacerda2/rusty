//! The path queue across a rebake, and run to run (#2087).

use glam::Vec3;

use super::{agent, level, spawn, DT};
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
