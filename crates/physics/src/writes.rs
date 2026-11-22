//! A body or collider is borrowed from rapier only when there is a value to write.
//! A disabled body's `position` is not read: rapier copies `next_position` into it only for an
//! active body. A static body is compared against `pushed`, not rapier's pose: the solver rounds a
//! rotation an ulp off.

use rapier3d::prelude::*;

use super::shape::build::EntityBodyState;
use rusty_components::RigidBodyComponent;

pub(super) fn state_differs(
    b: &RigidBody,
    snap: &EntityBodyState,
    class: RigidBodyType,
    pose: Pose,
    scale: f32,
    pushed: Option<&Pose>,
) -> bool {
    let placed = match pushed.filter(|_| snap.is_static) {
        Some(&pushed) => pushed != pose,
        None => (b.is_enabled() && *b.position() != pose) || *b.next_position() != pose,
    };
    let moved = if snap.kinematic || snap.is_static {
        placed
    } else {
        b.linvel() != snap.vel
            || b.angvel() != snap.angular_velocity
            || b.gravity_scale() != scale
            || !snap.damping.held_by(b)
    };
    b.body_type() != class || b.is_ccd_enabled() != snap.ccd_enabled || moved
}

pub(super) fn body_to_write(
    bodies: &mut RigidBodySet,
    handle: RigidBodyHandle,
    differs: impl FnOnce(&RigidBody) -> bool,
) -> Option<&mut RigidBody> {
    if !differs(bodies.get(handle)?) {
        return None;
    }
    // lint: allow(physics-borrow) the helper itself: `differs` found a value to write
    bodies.get_mut(handle)
}

/// [`body_to_write`] for a collider.
pub(super) fn collider_to_write(
    colliders: &mut ColliderSet,
    handle: ColliderHandle,
    differs: impl FnOnce(&Collider) -> bool,
) -> Option<&mut Collider> {
    if !differs(colliders.get(handle)?) {
        return None;
    }
    // lint: allow(physics-borrow) the helper itself: `differs` found a value to write
    colliders.get_mut(handle)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Damping {
    pub linear: f32,
    pub angular: f32,
}

impl Damping {
    pub(crate) fn of(rb: Option<&RigidBodyComponent>) -> Self {
        // Floored here as `mass` is where physics reads it: a file can hold a
        // negative drag, and rapier's `1 / (1 + dt·d)` would turn it into a gain.
        rb.map_or_else(Self::default, |r| Self {
            linear: r.drag.max(0.0),
            angular: r.angular_drag.max(0.0),
        })
    }

    pub(crate) fn held_by(self, body: &RigidBody) -> bool {
        body.linear_damping() == self.linear && body.angular_damping() == self.angular
    }
}
