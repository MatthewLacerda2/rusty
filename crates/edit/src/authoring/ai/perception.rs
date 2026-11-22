use rusty_components::AIPerceptionComponent as Perception;
use rusty_scene::rules::sanitize::{at_least, finite_or, saturated};

pub const MAX_SIGHT_ANGLE: f32 = 180.0;

pub fn set_sight_range(p: &mut Perception, range: f32) {
    p.sight_range = at_least(finite_or(range, p.sight_range), 0.0);
    p.lose_sight_range = p.lose_sight_range.max(p.sight_range);
}

/// Set the range a seen target is kept to, never below the sight range.
pub fn set_lose_sight_range(p: &mut Perception, range: f32) {
    p.lose_sight_range = at_least(finite_or(range, p.lose_sight_range), p.sight_range);
}

/// Set the view cone's half-angle in degrees, clamped to 0..=180 (`+inf` is 180).
pub fn set_sight_angle(p: &mut Perception, degrees: f32) {
    p.sight_angle = saturated(degrees, 0.0, MAX_SIGHT_ANGLE);
}

pub fn set_eye_height(p: &mut Perception, height: f32) {
    p.eye_height = finite_or(height, p.eye_height);
}

pub fn set_sight_mask(p: &mut Perception, mask: u32) {
    p.sight_mask = mask;
}

pub fn set_blocking_mask(p: &mut Perception, mask: u32) {
    p.blocking_mask = mask;
}

pub fn set_hearing_range(p: &mut Perception, range: f32) {
    p.hearing_range = at_least(finite_or(range, p.hearing_range), 0.0);
}

/// `seconds` (already finite) when it is a valid max age, not negative, else
/// `current`: a negative age is never floored to 0, which means forever.
fn age_or(seconds: f32, current: f32) -> f32 {
    if seconds < 0.0 {
        current
    } else {
        seconds
    }
}

pub fn set_sight_max_age(p: &mut Perception, seconds: f32) {
    p.sight_max_age = age_or(finite_or(seconds, p.sight_max_age), p.sight_max_age);
}

pub fn set_hearing_max_age(p: &mut Perception, seconds: f32) {
    p.hearing_max_age = age_or(finite_or(seconds, p.hearing_max_age), p.hearing_max_age);
}

pub fn set_damage_max_age(p: &mut Perception, seconds: f32) {
    p.damage_max_age = age_or(finite_or(seconds, p.damage_max_age), p.damage_max_age);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setters_keep_the_settings_well_formed() {
        let mut p = Perception::default();
        set_sight_range(&mut p, 30.0);
        assert_eq!(p.lose_sight_range, 30.0, "raised to the sight range");
        set_lose_sight_range(&mut p, 10.0);
        assert_eq!(p.lose_sight_range, 30.0, "never below the sight range");
        set_sight_range(&mut p, f32::NAN);
        set_sight_range(&mut p, -4.0);
        assert_eq!(p.sight_range, 0.0);
        set_sight_range(&mut p, f32::INFINITY);
        assert_eq!(
            p.sight_range, 0.0,
            "a non-finite range keeps the current one"
        );
        set_sight_angle(&mut p, 400.0);
        assert_eq!(p.sight_angle, MAX_SIGHT_ANGLE);
        set_sight_angle(&mut p, f32::NAN);
        assert_eq!(p.sight_angle, 0.0);
        set_eye_height(&mut p, f32::NEG_INFINITY);
        assert_eq!(p.eye_height, 1.6);
        set_hearing_max_age(&mut p, -1.0);
        assert_eq!(p.hearing_max_age, 3.0, "a negative age is not 0 (forever)");
        set_damage_max_age(&mut p, f32::NAN);
        assert_eq!(p.damage_max_age, 5.0);
        set_sight_mask(&mut p, 4);
        set_blocking_mask(&mut p, 2);
        assert_eq!((p.sight_mask, p.blocking_mask), (4, 2));
    }
}
