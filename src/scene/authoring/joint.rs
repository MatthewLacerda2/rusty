//! src/scene/authoring/joint.rs — Shared Joint-authoring ops (#449).
//!
//! The ONE place the engine mutates an entity's first-class `JointComponent`. The
//! editor's Joint card and the Lua `Joint.*` namespace both route every write
//! through these, so the bounds live once: angles stay within a half-turn, the
//! limit range is ordered, break thresholds are non-negative, and the axis is a
//! unit vector (a zero axis is ignored).
//!
//! Pure.

use glam::{Vec2, Vec3};

use super::sanitize::{clamped, finite3, non_negative};
use crate::components::{JointComponent, JointKind};

/// The widest angle (degrees) a limit may name: just short of a half-turn, where
/// a rotation's direction becomes ambiguous.
pub const MAX_ANGLE: f32 = 179.0;

/// Set which rotations the joint leaves free.
pub fn set_kind(j: &mut JointComponent, kind: JointKind) {
    j.kind = kind;
}

/// Set the entity it joins to (`None`: the world).
pub fn set_connected_body(j: &mut JointComponent, body: Option<u32>) {
    j.connected_body = body;
}

/// Set the pivot, in this entity's local space.
pub fn set_anchor(j: &mut JointComponent, anchor: Vec3) {
    j.anchor = finite3(anchor);
}

/// Set the pivot on the connected body (world space with no connected body).
pub fn set_connected_anchor(j: &mut JointComponent, anchor: Vec3) {
    j.connected_anchor = finite3(anchor);
}

/// Set whether the connected anchor is derived from `anchor` at build time.
pub fn set_auto_configure_connected_anchor(j: &mut JointComponent, auto: bool) {
    j.auto_configure_connected_anchor = auto;
}

/// Set the hinge / twist axis, normalized. A zero (or non-finite) axis names no
/// direction and is ignored.
// lint: allow(sanitize) `unit` refuses a NaN, infinite or zero axis; the old one stays
pub fn set_axis(j: &mut JointComponent, axis: Vec3) {
    if let Some(unit) = unit(axis) {
        j.axis = unit;
    }
}

/// Set a Ball joint's swing-1 axis, normalized. A zero (or non-finite) axis
/// names no direction and is ignored.
// lint: allow(sanitize) `unit` refuses a NaN, infinite or zero axis; the old one stays
pub fn set_swing_axis(j: &mut JointComponent, axis: Vec3) {
    if let Some(unit) = unit(axis) {
        j.swing_axis = unit;
    }
}

/// `axis` as a unit vector: as given when it already is one (to glam's
/// tolerance), so a saved axis loads and writes back bit for bit instead of
/// moving an ulp each time (#1418); else normalized; `None` for no direction.
fn unit(axis: Vec3) -> Option<Vec3> {
    if axis.is_normalized() {
        Some(axis)
    } else {
        axis.try_normalize()
    }
}

/// Set whether the angle limits apply.
pub fn set_use_limits(j: &mut JointComponent, use_limits: bool) {
    j.use_limits = use_limits;
}

/// Set the hinge / twist range in degrees, each end clamped to `±MAX_ANGLE` and
/// the pair ordered so `min <= max`.
pub fn set_limits(j: &mut JointComponent, min: f32, max: f32) {
    let (a, b) = (
        clamped(min, -MAX_ANGLE, MAX_ANGLE),
        clamped(max, -MAX_ANGLE, MAX_ANGLE),
    );
    j.limits = Vec2::new(a.min(b), a.max(b));
}

/// Set a Ball joint's swing-1 limit (degrees, about the swing axis), clamped to
/// `[0, MAX_ANGLE]`.
pub fn set_swing_limit(j: &mut JointComponent, degrees: f32) {
    j.swing_limit = clamped(degrees, 0.0, MAX_ANGLE);
}

/// Set a Ball joint's swing-2 limit (degrees, about `axis × swing_axis`),
/// clamped to `[0, MAX_ANGLE]`.
pub fn set_swing2_limit(j: &mut JointComponent, degrees: f32) {
    j.swing2_limit = clamped(degrees, 0.0, MAX_ANGLE);
}

/// Set the breaking force, floored at 0 (unbreakable).
pub fn set_break_force(j: &mut JointComponent, force: f32) {
    j.break_force = non_negative(force);
}

/// Set the breaking torque, floored at 0 (unbreakable).
pub fn set_break_torque(j: &mut JointComponent, torque: f32) {
    j.break_torque = non_negative(torque);
}

/// Set whether the two joined bodies collide with each other.
pub fn set_enable_collision(j: &mut JointComponent, enable: bool) {
    j.enable_collision = enable;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_and_clamp() {
        let mut j = JointComponent::default();
        set_kind(&mut j, JointKind::Hinge);
        set_axis(&mut j, Vec3::new(0.0, 3.0, 0.0));
        set_axis(&mut j, Vec3::ZERO);
        assert_eq!(j.axis, Vec3::Y, "normalized, and a zero axis is ignored");
        set_limits(&mut j, 400.0, -30.0);
        assert_eq!(j.limits, Vec2::new(-30.0, MAX_ANGLE), "clamped and ordered");
        set_swing_limit(&mut j, -10.0);
        set_swing2_limit(&mut j, 500.0);
        assert_eq!((j.swing_limit, j.swing2_limit), (0.0, MAX_ANGLE));
        set_swing_axis(&mut j, Vec3::new(0.0, 0.0, -2.0));
        set_swing_axis(&mut j, Vec3::NAN);
        assert_eq!(j.swing_axis, Vec3::NEG_Z);
        set_break_force(&mut j, -5.0);
        set_break_torque(&mut j, f32::NAN);
        assert_eq!((j.break_force, j.break_torque), (0.0, 0.0));
        set_connected_body(&mut j, Some(7));
        set_enable_collision(&mut j, true);
        assert_eq!(j.connected_body, Some(7));
        assert!(j.enable_collision && j.kind == JointKind::Hinge);
    }
}
