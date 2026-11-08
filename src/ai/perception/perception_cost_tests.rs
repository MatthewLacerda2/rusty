//! What perception costs on the Dust2-sized bench level (#1900): 30 perceivers
//! spread over `rooms(20)` and one target. The gate run checks the step works at
//! level scale under the ray budget; the timing is a release-build measurement
//! run by hand, in `dev::bench::perception_cost_tests` (a sim module may not read
//! a clock).

use glam::Vec3;

use super::{step, SightRays};
use crate::ai::query::query_cost_tests::level;
use crate::components::AIPerceptionComponent;
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// The bench level with 30 perceivers in a 6 × 5 grid of rooms, all looking for
/// one target standing in room (2, 2), and its physics world.
pub(crate) fn level_with_perceivers(n: usize) -> (Scene, PhysicsWorld, Vec<u32>) {
    let (mut scene, _nav, _) = level(n, 1.0);
    let mut bots = Vec::new();
    for i in 0..30 {
        let id = scene.add_entity(format!("bot_{i}"));
        let at = Vec3::new(
            5.0 + 10.0 * (i % 6) as f32,
            0.0,
            5.0 + 10.0 * (i / 6) as f32,
        );
        scene.world.transform_mut(id).expect("new").position = at;
        let p = AIPerceptionComponent {
            sight_angle: 180.0,
            ..Default::default()
        };
        scene.world.set_ai_perception(id, Some(p));
        bots.push(id);
    }
    let target = crate::ai::perception::test_world::target(&mut scene, Vec3::new(25.0, 0.0, 25.0));
    scene.world.set_layer(target, 0);
    let physics = PhysicsWorld::from_scene(&scene);
    (scene, physics, bots)
}

/// Step `ticks` perception ticks.
pub(crate) fn run(scene: &mut Scene, physics: &PhysicsWorld, rays: &mut SightRays, ticks: usize) {
    for _ in 0..ticks {
        step(scene, Some(physics), rays, 1.0 / 60.0);
    }
}

#[test]
fn thirty_perceivers_on_the_bench_level_see_the_target_near_them_only() {
    let (mut scene, physics, bots) = level_with_perceivers(20);
    let mut rays = SightRays::default();
    run(&mut scene, &physics, &mut rays, 2);
    let seeing: Vec<u32> = bots
        .iter()
        .copied()
        .filter(|&b| {
            !scene
                .world
                .ai_perception(b)
                .expect("bot")
                .memory
                .in_sight
                .is_empty()
        })
        .collect();
    assert!(!seeing.is_empty(), "the bot in the target's room sees it");
    assert!(
        seeing.len() < bots.len(),
        "walls hide it from the rest: {seeing:?}"
    );
}
