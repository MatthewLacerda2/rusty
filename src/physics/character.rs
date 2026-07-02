//! src/physics/character.rs — kinematic collide-and-slide.
//!
//! Kinematic bodies (player/enemy) are *driven* by writing their `Transform`, so
//! rapier never solves them out of static walls. This module routes that desired
//! motion through rapier's `KinematicCharacterController` (`move_shape`): given the
//! body's last solved pose and the script/input-set target, it computes a corrected
//! translation that stops at — and slides along — obstacles. The corrected pose is
//! the one we hand to `set_next_kinematic_position`, restoring wall blocking.
//!
//! rapier's controller deliberately does **not** apply gravity, so a `use_gravity`
//! kinematic body gets it fed here (#318): a fall speed accumulated across fixed
//! ticks is folded into the desired translation and zeroed on ground contact.
//!
//! Determinism: the controller is configured statically and `move_shape` is a pure
//! query over the current pipeline state at a fixed `dt`; no wall-clock or RNG.

use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::*;

/// A character controller tuned for the engine's walkers.
///
/// rapier's defaults already give us `slide: true` (collide-and-slide) and
/// `autostep: None`; the one knob is ground snapping. A gravity-driven body keeps
/// rapier's default `snap_to_ground` — snapping is the grounded detection that
/// stops a resting body from accumulating fall speed and jittering — while a body
/// that opts out of gravity disables it, so its vertical motion stays exactly what
/// the script asked for.
pub(super) fn controller(gravity: bool) -> KinematicCharacterController {
    let mut controller = KinematicCharacterController::default();
    if !gravity {
        controller.snap_to_ground = None;
    }
    controller
}

/// Gravity feed for one kinematic body's tick (#318). rapier's controller never
/// integrates gravity itself, so the engine carries a per-body fall speed across
/// ticks and folds it into the desired motion here.
pub(super) struct GravityFall {
    /// Downward acceleration magnitude (m/s², positive).
    pub accel: f32,
    /// Fall speed carried in from the previous tick (m/s, positive = falling).
    pub speed: f32,
}

/// Borrowed rapier state the controller queries against. Bundled so the resolve
/// entry point stays under the 6-argument cap.
pub(super) struct RapierRefs<'a> {
    pub bodies: &'a RigidBodySet,
    pub colliders: &'a ColliderSet,
    pub queries: &'a QueryPipeline,
}

/// Resolve a kinematic body's move toward `target` and return the corrected next
/// pose plus the fall speed to carry into the next tick. The desired translation
/// (last solved pos -> target, plus the gravity term when `gravity` is given) is
/// run through the controller (collide-and-slide); the target rotation is kept
/// verbatim, since the controller only resolves translation. The body is excluded
/// from the query so it never collides with itself.
pub(super) fn corrected_next_pose(
    refs: RapierRefs<'_>,
    body_handle: RigidBodyHandle,
    target: Isometry<Real>,
    dt: Real,
    gravity: Option<GravityFall>,
) -> (Isometry<Real>, f32) {
    let body = &refs.bodies[body_handle];
    let current = *body.position();
    let mut desired = target.translation.vector - current.translation.vector;
    // Semi-implicit Euler: accelerate the carried fall speed first, then pull the
    // desired motion down by it on top of whatever the script asked for.
    let mut fall_speed = gravity.as_ref().map_or(0.0, |g| g.speed + g.accel * dt);
    desired.y -= fall_speed * dt;
    let controller = controller(gravity.is_some());
    let shape = body
        .colliders()
        .first()
        .and_then(|&h| refs.colliders.get(h))
        .map(|c| c.shared_shape().clone());

    let translation = match shape {
        Some(shape) => {
            // Sensors are trigger volumes, not walls: they must never block the
            // move, or a character could not enter the volume whose
            // OnTriggerEnter it is meant to fire (#310).
            let filter = QueryFilter::default()
                .exclude_sensors()
                .exclude_rigid_body(body_handle);
            let movement = controller.move_shape(
                dt,
                refs.bodies,
                refs.colliders,
                refs.queries,
                shape.as_ref(),
                &current,
                desired,
                filter,
                |_| {},
            );
            if movement.grounded {
                // Standing on the floor: stop accumulating, so stepping off a
                // ledge starts a fresh fall instead of an instant plummet.
                fall_speed = 0.0;
            }
            movement.translation
        }
        // No collider shape to sweep: fall back to the raw desired motion.
        None => desired,
    };

    let pose = Isometry::from_parts(
        (current.translation.vector + translation).into(),
        target.rotation,
    );
    (pose, fall_speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_snaps_to_ground_only_under_gravity() {
        // Without gravity, kinematic bodies keep the exact vertical motion scripts
        // drive, so ground-snapping must be off.
        let free = controller(false);
        assert!(free.snap_to_ground.is_none(), "snap off without gravity");
        // With gravity, snapping is the grounded detection that keeps a resting
        // body from accumulating fall speed.
        let falling = controller(true);
        assert!(falling.snap_to_ground.is_some(), "snap on under gravity");
        // The shared config we rely on (these happen to match rapier defaults).
        for c in [free, falling] {
            assert!(c.slide, "collide-and-slide stays on");
            assert!(c.autostep.is_none(), "autostep stays off");
        }
    }
}
