//! src/physics/character.rs — kinematic collide-and-slide.
//!
//! Kinematic bodies (player/enemy) are *driven* by writing their `Transform`, so
//! rapier never solves them out of static walls. This module routes that desired
//! motion through rapier's `KinematicCharacterController` (`move_shape`): given the
//! body's last solved pose and the script/input-set target, it computes a corrected
//! translation that stops at — and slides along — obstacles. The corrected pose is
//! the one we hand to `set_next_kinematic_position`, restoring wall blocking.
//!
//! Determinism: the controller is configured statically and `move_shape` is a pure
//! query over the current pipeline state at a fixed `dt`; no wall-clock or RNG.

use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::*;

/// A character controller tuned for the engine's flat-ground walkers.
///
/// `slide` is on (collide-and-slide); ground-snapping and autostep are off so a
/// horizontally-driven body keeps exactly the vertical motion the script asked for
/// (the existing kinematic bodies opt out of gravity).
pub(super) fn controller() -> KinematicCharacterController {
    KinematicCharacterController {
        slide: true,
        autostep: None,
        snap_to_ground: None,
        ..KinematicCharacterController::default()
    }
}

/// Borrowed rapier state the controller queries against. Bundled so the resolve
/// entry point stays under the 6-argument cap.
pub(super) struct RapierRefs<'a> {
    pub bodies: &'a RigidBodySet,
    pub colliders: &'a ColliderSet,
    pub queries: &'a QueryPipeline,
}

/// Resolve a kinematic body's move toward `target` and return the corrected next
/// pose. The desired translation (last solved pos -> target) is run through the
/// controller (collide-and-slide); the target rotation is kept verbatim, since the
/// controller only resolves translation. The body is excluded from the query so it
/// never collides with itself.
pub(super) fn corrected_next_pose(
    controller: &KinematicCharacterController,
    refs: RapierRefs<'_>,
    body_handle: RigidBodyHandle,
    target: Isometry<Real>,
    dt: Real,
) -> Isometry<Real> {
    let body = &refs.bodies[body_handle];
    let current = *body.position();
    let desired = target.translation.vector - current.translation.vector;
    let shape = body
        .colliders()
        .first()
        .and_then(|&h| refs.colliders.get(h))
        .map(|c| c.shared_shape().clone());

    let translation = match shape {
        Some(shape) => {
            let filter = QueryFilter::default().exclude_rigid_body(body_handle);
            controller
                .move_shape(
                    dt,
                    refs.bodies,
                    refs.colliders,
                    refs.queries,
                    shape.as_ref(),
                    &current,
                    desired,
                    filter,
                    |_| {},
                )
                .translation
        }
        // No collider shape to sweep: fall back to the raw desired motion.
        None => desired,
    };

    Isometry::from_parts(
        (current.translation.vector + translation).into(),
        target.rotation,
    )
}
