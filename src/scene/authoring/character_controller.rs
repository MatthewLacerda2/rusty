//! src/scene/authoring/character_controller.rs — Shared CharacterController
//! authoring ops (#451).
//!
//! The ONE place the engine mutates an entity's `CharacterControllerComponent`'s
//! authored fields. The editor's card and the Lua `CharacterController.*` setters
//! both route every write through these, so the bounds live once: sizes stay
//! positive, the slope limit stays within a half-turn and the skin never reaches
//! zero (the collide-and-slide sweep needs a gap to keep). The runtime fields
//! (`is_grounded`, the flags, the ground normal) are written only by `Move`.
//!
//! Allowed deps: components (the component data). Pure.

use glam::Vec3;

use crate::components::CharacterControllerComponent;

/// The smallest radius / skin a capsule may have: a zero would make the sweep
/// degenerate.
pub const MIN_SIZE: f32 = 1e-4;

/// Set the capsule's full height, floored at 0 (under `2 * radius` it is a
/// sphere).
pub fn set_height(c: &mut CharacterControllerComponent, height: f32) {
    c.height = non_negative(height);
}

/// Set the capsule's radius, floored at [`MIN_SIZE`].
pub fn set_radius(c: &mut CharacterControllerComponent, radius: f32) {
    c.radius = at_least(radius, MIN_SIZE);
}

/// Set the capsule's centre, in local space. A non-finite centre is ignored.
pub fn set_center(c: &mut CharacterControllerComponent, center: Vec3) {
    if center.is_finite() {
        c.center = center;
    }
}

/// Set the tallest step a move climbs, floored at 0 (no stepping).
pub fn set_step_offset(c: &mut CharacterControllerComponent, step: f32) {
    c.step_offset = non_negative(step);
}

/// Set the steepest walkable slope, in degrees, clamped to `[0, 180]` (Unity's
/// range).
pub fn set_slope_limit(c: &mut CharacterControllerComponent, degrees: f32) {
    c.slope_limit = non_negative(degrees).min(180.0);
}

/// Set the contact gap, floored at [`MIN_SIZE`].
pub fn set_skin_width(c: &mut CharacterControllerComponent, skin: f32) {
    c.skin_width = at_least(skin, MIN_SIZE);
}

/// Set the shortest move that does anything, floored at 0.
pub fn set_min_move_distance(c: &mut CharacterControllerComponent, distance: f32) {
    c.min_move_distance = non_negative(distance);
}

/// `v` floored at 0; a NaN reads as 0.
fn non_negative(v: f32) -> f32 {
    at_least(v, 0.0)
}

/// `v` floored at `min`; a NaN reads as `min`.
fn at_least(v: f32, min: f32) -> f32 {
    if v.is_nan() {
        min
    } else {
        v.max(min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setters_clamp_to_their_bounds() {
        let mut c = CharacterControllerComponent::default();
        set_height(&mut c, -1.0);
        assert_eq!(c.height, 0.0);
        set_radius(&mut c, 0.0);
        assert_eq!(c.radius, MIN_SIZE);
        set_slope_limit(&mut c, 400.0);
        assert_eq!(c.slope_limit, 180.0);
        set_skin_width(&mut c, f32::NAN);
        assert_eq!(c.skin_width, MIN_SIZE);
        set_step_offset(&mut c, -0.2);
        assert_eq!(c.step_offset, 0.0);
        set_min_move_distance(&mut c, -3.0);
        assert_eq!(c.min_move_distance, 0.0);
        set_center(&mut c, Vec3::new(0.0, 1.0, 0.0));
        set_center(&mut c, Vec3::NAN);
        assert_eq!(c.center, Vec3::Y, "a non-finite centre is ignored");
    }
}
