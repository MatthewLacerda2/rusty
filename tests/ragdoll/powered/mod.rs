//! The powered partial ragdoll (#1895): a branch switched to physics alone, its
//! joints driven toward the animated pose, its drawn pose blended, and the
//! one-call hit reaction, in Play, run twice the same.

mod blend;
mod drive;
mod hit;
mod ik;
mod mirror;
mod partial;
mod react;

use glam::{Quat, Vec3};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

use super::rig;

/// No body's velocity is known: every switched body starts at rest.
pub fn at_rest(_: u32) -> Option<(Vec3, Vec3)> {
    None
}

/// Step `physics` `n` fixed ticks and pose the ragdoll bones after each.
pub fn step(scene: &mut Scene, physics: &mut PhysicsWorld, n: usize) {
    for _ in 0..n {
        physics.step(scene, 1.0 / 60.0);
        physics.pose_bones(scene);
    }
}

/// `name`'s world rotation.
pub fn turn(scene: &Scene, rig: u32, name: &str) -> Quat {
    scene.world.world_rotation(rig::bone(scene, rig, name))
}
