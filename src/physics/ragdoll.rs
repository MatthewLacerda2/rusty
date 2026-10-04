//! src/physics/ragdoll.rs — what a ragdoll needs from the physics world (#466).
//!
//! A ragdoll is ordinary components — a `Rigidbody` and a `Joint` on each bone,
//! the bone's hitbox as its collider — so the step simulates it like any other
//! body. Three things are specific to bodies on bones:
//!
//! - **The post-animation writer.** A tick runs the step, then the Animator. The
//!   step writes each ragdolled bone's pose, but the Animator then rewrites the
//!   bones it keys — and any animated bone *above* a ragdolled one drags it along.
//!   [`PhysicsWorld::pose_bones`] runs right after the Animator and puts every
//!   bone carried by a dynamic body back where its body is, parents first, so
//!   `LateUpdate` and skinning see the simulated pose.
//! - **The handoff's velocities.** While animated, a bone's body is kinematic and
//!   rapier derives its velocity from how far the Animator moved it.
//!   [`PhysicsWorld::body_velocity`] reads it so a ragdoll starts moving the way
//!   the animation was, instead of freezing and dropping.
//! - **Impulses at a point.** [`PhysicsWorld::impulse_response`] turns an impulse
//!   at a world point into the linear and angular velocity change it causes,
//!   from the body's own mass, centre of mass and inertia.

use glam::Vec3;

use super::world::PhysicsWorld;
use crate::scene::Scene;

impl PhysicsWorld {
    /// Write every bone carried by a dynamic body back to its body's pose, parents
    /// before children. A scene without dynamic bones costs one set build.
    pub fn pose_bones(&self, scene: &mut Scene) {
        let bones = scene.bone_ids();
        let mut owners: Vec<(usize, u32)> = self
            .plan
            .keys()
            .filter(|id| bones.contains(id))
            .filter(|id| {
                self.id_to_body
                    .get(id)
                    .and_then(|&h| self.bodies.get(h))
                    .is_some_and(|b| b.is_dynamic())
            })
            .map(|&id| (depth(scene, id), id))
            .collect();
        owners.sort_unstable();
        for (_, id) in owners {
            self.write_back(scene, id);
        }
    }

    /// The linear and angular velocity of the body `id` owns, as rapier has it —
    /// for a kinematic body, the motion it was driven through last step.
    pub fn body_velocity(&self, id: u32) -> Option<(Vec3, Vec3)> {
        let body = self.bodies.get(*self.id_to_body.get(&id)?)?;
        Some((body.linvel(), body.angvel()))
    }

    /// The `(linear, angular)` velocity change an `impulse` (N·s) at world
    /// `point` gives the body `id` owns: `J / m`, and the inverse world inertia
    /// times the torque impulse about the centre of mass. Read from the body's
    /// mass properties, which rapier keeps for kinematic bodies too, so it holds
    /// on the tick a ragdoll is switched on. `None` without a massive body.
    pub fn impulse_response(&self, id: u32, impulse: Vec3, point: Vec3) -> Option<(Vec3, Vec3)> {
        let body = self.bodies.get(*self.id_to_body.get(&id)?)?;
        let props = &body.mass_properties().local_mprops;
        if props.inv_mass <= 0.0 {
            return None;
        }
        let com = body.position() * props.local_com;
        let j = impulse;
        let torque = (point - com).cross(j);
        let dw = props.world_inv_inertia(body.rotation()) * torque;
        Some((impulse * props.inv_mass, dw))
    }
}

/// How many ancestors `id` has.
fn depth(scene: &Scene, id: u32) -> usize {
    std::iter::successors(scene.world.parent_id(id), |&p| scene.world.parent_id(p)).count()
}
