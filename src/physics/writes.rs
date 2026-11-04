//! src/physics/writes.rs — what a sync pass may borrow from rapier (#1798): a body or
//! collider only when there is a value to write. A borrow alone changes what the next
//! step does, so these are the one way in (the `physics-borrow` rule holds it).

use rapier3d::prelude::*;

use super::shape::build::EntityBodyState;

/// Whether `PhysicsWorld::apply_body_state` has anything to write: the
/// comparisons its setters make before they write. A disabled body's `position`
/// is not read: rapier copies `next_position` into it only for an active body,
/// so a disabled kinematic body that moved would differ forever and be borrowed
/// by every full pass but no incremental one. Re-enabling it is safe, since
/// `sync_enabled` runs before this in the same pass.
pub(super) fn state_differs(
    b: &RigidBody,
    snap: &EntityBodyState,
    class: RigidBodyType,
    pose: Pose,
    scale: f32,
) -> bool {
    let placed = (b.is_enabled() && *b.position() != pose) || *b.next_position() != pose;
    let moved = if snap.kinematic || snap.is_static {
        placed
    } else {
        b.linvel() != snap.vel || b.angvel() != snap.angular_velocity || b.gravity_scale() != scale
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
