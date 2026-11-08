//! src/physics/character/hits.rs — what one `Move` touched, per collider:
//! Unity's `ControllerColliderHit`, the payload of `OnControllerColliderHit`
//! (#1879).
//!
//! rapier's controller reports every contact of its collide-and-slide sweep in
//! the order the sweep met them. A collider met several times (a wall slid
//! along, a floor pressed into on each slide) is reported once, at its first
//! contact, so a script hears each collider once per `Move`, in sweep order —
//! a total order, so equal runs give equal lists.

use glam::Vec3;
use rapier3d::control::CharacterCollision;

use crate::physics::world::PhysicsWorld;

/// One collider a CharacterController's `Move` touched (Unity's
/// `ControllerColliderHit`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerHit {
    /// The entity whose collider was hit (Unity's `hit.collider`).
    pub id: u32,
    /// Where it was hit, in world space: the point on the struck collider.
    pub point: Vec3,
    /// The struck surface's outward normal, in world space.
    pub normal: Vec3,
    /// The direction the `Move` displaced the controller (unit length).
    pub move_direction: Vec3,
    /// How far the controller had travelled in this `Move` when it hit.
    pub move_length: f32,
}

impl PhysicsWorld {
    /// The first contact with each collider in `collisions`, in sweep order,
    /// for a `Move` of `motion`. A collider with no entity (none today) is
    /// skipped.
    pub(super) fn controller_hits(
        &self,
        collisions: &[CharacterCollision],
        motion: Vec3,
    ) -> Vec<ControllerHit> {
        let move_direction = motion.normalize_or_zero();
        let mut hits: Vec<ControllerHit> = Vec::new();
        for c in collisions {
            let Some(&id) = self.collider_to_id.get(&c.handle) else {
                continue;
            };
            if hits.iter().any(|h| h.id == id) {
                continue;
            }
            hits.push(ControllerHit {
                id,
                point: c.hit.witness1,
                normal: c.hit.normal1.normalize_or_zero(),
                move_direction,
                move_length: c.translation_applied.length(),
            });
        }
        hits
    }
}
