pub mod bench;
mod bodies;
mod body;
mod composite;
mod cost;
#[cfg(test)]
mod cost_tests;
mod hits;
mod normals;
mod parts;
mod pass;
mod sweep;
mod view;

pub(super) use bodies::ControllerBodies;
pub(super) use body::{character_collider_inputs, upright_offset};
pub use hits::ControllerHit;

use glam::Vec3;
use rapier3d::prelude::*;

use super::compound::{world_pose, world_to_local};
use super::query::is_live;
use super::world::PhysicsWorld;
use rusty_components::{capsule_dims, CapsuleAxis, CharacterControllerComponent};
use rusty_scene::Scene;

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterMove {
    pub flags: u8,
    pub grounded: bool,
    pub ground_normal: Vec3,
    pub hits: Vec<ControllerHit>,
}

/// `motion` is world space, metres; with no physics world (edit mode) it moves unobstructed.
/// It pushes nothing: rapier's `solve_character_collision_impulses` stays uncalled.
pub fn move_character(
    physics: Option<&PhysicsWorld>,
    scene: &mut Scene,
    id: u32,
    motion: Vec3,
) -> Option<CharacterMove> {
    let cc = scene.world.character_controller(id)?.clone();
    if !motion.is_finite() || motion.length() < cc.min_move_distance {
        // Unity: a move under the minimum distance does nothing at all.
        return Some(CharacterMove {
            flags: 0,
            grounded: cc.is_grounded,
            ground_normal: cc.ground_normal,
            hits: Vec::new(),
        });
    }
    let (translation, result) = match physics {
        Some(world) => world.sweep(scene, id, &cc, motion)?,
        None => (motion, CharacterMove::airborne(0, Vec::new())),
    };
    let pose = world_pose(scene, id)?;
    let (local, _) = world_to_local(scene, id, pose.pos + translation, pose.rot);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = local;
    }
    if let Some(mut c) = scene.world.character_controller_mut(id) {
        c.is_grounded = result.grounded;
        c.collision_flags = result.flags;
        c.ground_normal = result.ground_normal;
    }
    Some(result)
}

/// `true` with no physics world (edit mode).
pub fn can_stand(physics: Option<&PhysicsWorld>, scene: &Scene, id: u32, height: f32) -> bool {
    let Some(cc) = scene.world.character_controller(id).map(|c| c.clone()) else {
        return false;
    };
    let Some(world) = physics else {
        return true;
    };
    let (Some(now), Some(next)) = (
        capsule(scene, id, &cc, cc.height),
        capsule(scene, id, &cc, height),
    ) else {
        return false;
    };
    let center = Vec3::new(
        now.center.x,
        now.bottom() + next.half_extent(),
        now.center.z,
    );
    let live = |_: ColliderHandle, c: &Collider| is_live(&world.bodies, c);
    let filter = world.character_filter(scene, id, &live);
    let shape = next.shape();
    let pos = Pose::from_translation(center);
    let blocked = world
        .queries(filter)
        .intersect_shape(pos, &shape)
        .next()
        .is_some();
    !blocked
}

impl CharacterMove {
    pub(super) fn airborne(flags: u8, hits: Vec<ControllerHit>) -> Self {
        Self {
            flags,
            grounded: false,
            ground_normal: Vec3::Y,
            hits,
        }
    }
}

pub(super) struct Capsule {
    pub(super) center: Vec3,
    pub(super) half_segment: f32,
    pub(super) radius: f32,
}

impl Capsule {
    pub(super) fn shape(&self) -> rapier3d::parry::shape::Capsule {
        rapier3d::parry::shape::Capsule::new_y(self.half_segment, self.radius)
    }

    fn half_extent(&self) -> f32 {
        self.half_segment + self.radius
    }

    fn bottom(&self) -> f32 {
        self.center.y - self.half_extent()
    }
}

pub(super) fn capsule(
    scene: &Scene,
    id: u32,
    cc: &CharacterControllerComponent,
    height: f32,
) -> Option<Capsule> {
    let pose = world_pose(scene, id)?;
    let (half_segment, radius) = capsule_dims(cc.radius, height, CapsuleAxis::Y, pose.scale);
    Some(Capsule {
        center: pose.pos + pose.rot * (cc.center * pose.scale),
        half_segment,
        radius,
    })
}
