use glam::Vec3;

use super::{step, SightRays};
use crate::perception::test_world::target;
use crate::query::bench::blocks;
use rusty_components::AIPerceptionComponent;
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

/// The bots' layer, not `PLAYER`'s: a `sight_mask` of the target's layer alone
/// skips the other 29 bots.
const BOT: u8 = 4;

/// The bench level with 30 perceivers in a 6 × 5 grid of rooms, each a 1.8 m
/// body, looking for `sight_mask` (the target's layer, or `u32::MAX`), one target
/// standing in room (2, 2), and its physics world.
pub fn level_with_perceivers(n: usize, sight_mask: u32) -> (Scene, PhysicsWorld, Vec<u32>) {
    // Perception reads the colliders only: no navmesh is baked.
    let mut scene = blocks(n);
    let mut bots = Vec::new();
    for i in 0..30 {
        let at = Vec3::new(
            5.0 + 10.0 * (i % 6) as f32,
            0.0,
            5.0 + 10.0 * (i / 6) as f32,
        );
        // A body like the target's, eyes 0.7 m over its centre (1.6 m off the floor).
        let id = target(&mut scene, at);
        scene.world.set_name(id, format!("bot_{i}"));
        scene.world.set_layer(id, BOT);
        let p = AIPerceptionComponent {
            sight_angle: 180.0,
            eye_height: 0.7,
            sight_mask,
            ..Default::default()
        };
        scene.world.set_ai_perception(id, Some(p));
        bots.push(id);
    }
    target(&mut scene, Vec3::new(25.0, 0.0, 25.0));
    let physics = PhysicsWorld::from_scene(&scene);
    (scene, physics, bots)
}

/// Step `ticks` perception ticks.
pub fn run(scene: &mut Scene, physics: &PhysicsWorld, rays: &mut SightRays, ticks: usize) {
    for _ in 0..ticks {
        step(scene, Some(physics), rays, 1.0 / 60.0);
    }
}
