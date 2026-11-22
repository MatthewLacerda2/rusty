use glam::Vec3;

use rusty_components::CharacterControllerComponent;
use rusty_scene::rules::sanitize::{at_least, finite3_or, non_negative};

pub const MIN_SIZE: f32 = 1e-4;

pub fn set_height(c: &mut CharacterControllerComponent, height: f32) {
    c.height = non_negative(height);
}

pub fn set_radius(c: &mut CharacterControllerComponent, radius: f32) {
    c.radius = at_least(radius, MIN_SIZE);
}

pub fn set_center(c: &mut CharacterControllerComponent, center: Vec3) {
    c.center = finite3_or(center, c.center);
}

pub fn set_step_offset(c: &mut CharacterControllerComponent, step: f32) {
    c.step_offset = non_negative(step);
}

/// Set the steepest walkable slope, in degrees, clamped to `[0, 180]` (Unity's
/// range).
pub fn set_slope_limit(c: &mut CharacterControllerComponent, degrees: f32) {
    c.slope_limit = non_negative(degrees).min(180.0);
}

pub fn set_skin_width(c: &mut CharacterControllerComponent, skin: f32) {
    c.skin_width = at_least(skin, MIN_SIZE);
}

pub fn set_min_move_distance(c: &mut CharacterControllerComponent, distance: f32) {
    c.min_move_distance = non_negative(distance);
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
        set_skin_width(&mut c, f32::INFINITY);
        assert_eq!(c.skin_width, MIN_SIZE, "+inf is refused like NaN");
        set_slope_limit(&mut c, f32::INFINITY);
        assert_eq!(c.slope_limit, 0.0);
        set_step_offset(&mut c, -0.2);
        assert_eq!(c.step_offset, 0.0);
        set_min_move_distance(&mut c, -3.0);
        assert_eq!(c.min_move_distance, 0.0);
        set_center(&mut c, Vec3::new(0.0, 1.0, 0.0));
        set_center(&mut c, Vec3::NAN);
        assert_eq!(c.center, Vec3::Y, "a non-finite centre is ignored");
    }
}
