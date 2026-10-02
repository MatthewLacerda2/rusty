//! src/physics/character.rs — the CharacterController's collide-and-slide (#451).
//!
//! A CharacterController moves only when a script calls `Move` with a
//! displacement. That move runs rapier's `KinematicCharacterController` over the
//! live query pipeline, configured from the component: the skin is the gap kept
//! to obstacles, the step offset is rapier's autostep height (and how far a
//! downhill move snaps back to the ground), and the slope limit is both the
//! steepest climb and where sliding starts. The corrected displacement is written
//! straight to the Transform, so the move takes effect at once and the next
//! physics tick carries the kinematic body there. No gravity is applied: scripts
//! own it (Unity's `CharacterController.Move`).
//!
//! Plain kinematic bodies are pure movers — they go exactly where their Transform
//! says and never collide-and-slide (Unity's kinematic Rigidbody).
//!
//! The capsule stays upright whatever the entity's rotation, and is the entity's
//! collider in the physics world, so other characters, rays and triggers meet the
//! same shape the move sweeps. [`PhysicsWorld::sync_characters`] keeps that
//! collider's size and offset current each tick (crouching resizes it).
//!
//! Determinism: the controller is a pure query over the pipeline state at the
//! fixed dt; no wall-clock or RNG.

mod body;
mod sweep;

pub(super) use body::{character_collider_inputs, upright_offset};

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::capsule_dims;
use super::compound::{world_pose, world_to_local};
use super::query::is_live;
use super::world::PhysicsWorld;
use crate::components::{CapsuleAxis, CharacterControllerComponent};
use crate::scene::Scene;

/// ended on the ground, and the ground's normal (`Vec3::Y` when airborne).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterMove {
    pub flags: u8,
    pub grounded: bool,
    pub ground_normal: Vec3,
}

/// Move character `id` by `motion` (world space, metres), colliding and sliding
/// against the live physics world; with none (edit mode) it moves unobstructed.
/// Writes the Transform and the component's grounded state. `None` when `id`
/// has no CharacterController.
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
        });
    }
    let (translation, result) = match physics {
        Some(world) => world.sweep(scene, id, &cc, motion)?,
        None => (motion, CharacterMove::airborne(0)),
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

/// Whether character `id`'s capsule, resized to `height` with its bottom kept
/// where it is, would overlap nothing — the "can I stand up here?" check before
/// growing out of a crouch. `false` without a CharacterController; `true` with
/// no physics world (edit mode has nothing to collide with).
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
    let pos = Isometry::translation(center.x, center.y, center.z);
    world
        .query_pipeline
        .intersection_with_shape(&world.bodies, &world.colliders, &pos, &shape, filter)
        .is_none()
}

impl CharacterMove {
    pub(super) fn airborne(flags: u8) -> Self {
        Self {
            flags,
            grounded: false,
            ground_normal: Vec3::Y,
        }
    }
}

/// A character's capsule in world space: its centre, inner half-segment and
/// radius (scale baked in).
pub(super) struct Capsule {
    pub(super) center: Vec3,
    pub(super) half_segment: f32,
    pub(super) radius: f32,
}

impl Capsule {
    pub(super) fn shape(&self) -> rapier3d::parry::shape::Capsule {
        rapier3d::parry::shape::Capsule::new_y(self.half_segment, self.radius)
    }

    /// Centre to the bottom (or top) of the capsule.
    fn half_extent(&self) -> f32 {
        self.half_segment + self.radius
    }

    fn bottom(&self) -> f32 {
        self.center.y - self.half_extent()
    }
}

/// `id`'s capsule at `height`, from its live world pose.
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
