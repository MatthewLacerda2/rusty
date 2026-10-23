//! src/physics/joints/frame.rs — the world frame a live joint was built on, read
//! back from rapier: what a test or a debug view checks a joint's axes against
//! (#1125), without recomputing it from the component.

use glam::Quat;

use super::super::world::PhysicsWorld;

impl PhysicsWorld {
    /// The world rotation of joint entity `id`'s frame as physics holds it now
    /// (its X is the hinge / twist axis, its Y the swing axis): the first body's
    /// pose times the joint's frame on it. `None` when `id` has no live joint.
    /// Test seam for `tests/ragdoll/mirrored.rs`.
    pub fn joint_world_frame(&self, id: u32) -> Option<Quat> {
        let joint = self.impulse_joints.get(self.joints.get(&id)?.handle)?;
        let body = self.bodies.get(joint.body1())?;
        Some(
            (*body.position() * joint.data.local_frame1)
                .rotation
                .normalize(),
        )
    }
}
