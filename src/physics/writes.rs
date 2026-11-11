//! src/physics/writes.rs — what a sync pass may borrow from rapier (#1798): a body or
//! collider only when there is a value to write. A borrow alone changes what the next
//! step does, so these are the one way in (the `physics-borrow` rule holds it).

use rapier3d::prelude::*;

use super::shape::build::EntityBodyState;
use crate::components::RigidBodyComponent;

/// Whether `PhysicsWorld::apply_body_state` has anything to write: the
/// comparisons its setters make before they write. A disabled body's `position`
/// is not read: rapier copies `next_position` into it only for an active body,
/// so a disabled kinematic body that moved would differ forever and be borrowed
/// by every full pass but no incremental one. Re-enabling it is safe, since
/// `sync_enabled` runs before this in the same pass.
///
/// A static body is compared against `pushed`, the pose last pushed to it
/// (`Tracked::static_poses`), when there is one, not against rapier's: the solver
/// rewrites the pose of a body still in its active set, a body just turned static
/// included, and rounds a rotation an ulp off (#1842). Compared with rapier's
/// pose, the body would be re-pushed every step, and each push wakes whatever
/// rests on it.
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

/// `handle`'s body, borrowed mutably only when `differs` finds a value to
/// write. rapier lists every body and collider borrowed mutably as modified,
/// written or not, and the order of that list is the order the next step meets
/// new contact pairs in, so the order the solver works them. A sync that
/// borrowed every body (the full pass) stepped differently from one that
/// borrowed only the bodies a change named (#1798). Every sync pass borrows
/// through this or [`collider_to_write`].
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

/// A body's linear and angular damping: its Rigidbody's `drag` and
/// `angular_drag`, zero without one (a static or lone-collider body never moves).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::physics) struct Damping {
    pub linear: f32,
    pub angular: f32,
}

impl Damping {
    pub(in crate::physics) fn of(rb: Option<&RigidBodyComponent>) -> Self {
        rb.map_or_else(Self::default, |r| Self {
            linear: r.drag,
            angular: r.angular_drag,
        })
    }

    /// Whether rapier's `body` already carries this damping.
    pub(in crate::physics) fn held_by(self, body: &RigidBody) -> bool {
        body.linear_damping() == self.linear && body.angular_damping() == self.angular
    }
}
