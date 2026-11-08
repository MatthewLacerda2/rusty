//! src/scene/authoring/ai/perception.rs — Shared AIPerception authoring ops (#1900).
//!
//! The ONE place the engine mutates an entity's `AIPerceptionComponent` settings.
//! The editor's card, `Component.Set` and the Lua `AIPerception.Set*` setters all
//! route through these, so the rules live once: every distance and age is finite
//! and non-negative (a bad input keeps the current value), the lose-sight range
//! never falls below the sight range, and the view half-angle stays in 0..=180°.
//! The agent's memory is the senses' own (`ai::perception`), never written here.
//!
//! Pure.

use crate::components::AIPerceptionComponent as Perception;
use crate::scene::authoring::sanitize::{at_least, finite_or, saturated};

/// The widest view half-angle: 180° sees all round.
pub const MAX_SIGHT_ANGLE: f32 = 180.0;

/// Set the range sight starts at; the lose-sight range is raised to it if below.
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

/// Set the eye's height above the entity's origin. A non-finite height is ignored.
pub fn set_eye_height(p: &mut Perception, height: f32) {
    p.eye_height = finite_or(height, p.eye_height);
}

/// Set the layers a sight target may be on.
pub fn set_sight_mask(p: &mut Perception, mask: u32) {
    p.sight_mask = mask;
}

/// Set the layers that block a sight ray.
pub fn set_blocking_mask(p: &mut Perception, mask: u32) {
    p.blocking_mask = mask;
}

/// Set how far a noise of loudness 1 is heard.
pub fn set_hearing_range(p: &mut Perception, range: f32) {
    p.hearing_range = at_least(finite_or(range, p.hearing_range), 0.0);
}

/// Set how long a seen target is remembered out of sight (0 = forever).
pub fn set_sight_max_age(p: &mut Perception, seconds: f32) {
    p.sight_max_age = at_least(finite_or(seconds, p.sight_max_age), 0.0);
}

/// Set how long a heard noise is remembered (0 = forever).
pub fn set_hearing_max_age(p: &mut Perception, seconds: f32) {
    p.hearing_max_age = at_least(finite_or(seconds, p.hearing_max_age), 0.0);
}

/// Set how long a damage stimulus is remembered (0 = forever).
pub fn set_damage_max_age(p: &mut Perception, seconds: f32) {
    p.damage_max_age = at_least(finite_or(seconds, p.damage_max_age), 0.0);
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
        assert_eq!(p.hearing_max_age, 0.0);
        set_damage_max_age(&mut p, f32::NAN);
        assert_eq!(p.damage_max_age, 5.0);
        set_sight_mask(&mut p, 4);
        set_blocking_mask(&mut p, 2);
        assert_eq!((p.sight_mask, p.blocking_mask), (4, 2));
    }
}
