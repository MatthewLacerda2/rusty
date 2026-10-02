//! src/scene/authoring/nav_obstacle.rs — Shared NavMeshObstacle authoring ops (#456).
//!
//! The ONE place the engine mutates an entity's `NavMeshObstacleComponent`'s
//! authored fields. The editor's card and the Lua `NavMeshObstacle.*` setters both
//! route every write through these, so the bounds live once: sizes stay positive
//! and the carving thresholds never go negative. The runtime fields (the carve
//! pose, stationary time, velocity) are written only by the play-mode tick.
//!
//! Pure.

use glam::Vec3;

use crate::components::{NavMeshObstacleComponent as Obstacle, ObstacleShape};

/// The smallest extent a box side, radius or height may have.
pub const MIN_SIZE: f32 = 1e-3;

/// Enable or disable the obstacle (Unity's `enabled`).
pub fn set_active(o: &mut Obstacle, active: bool) {
    o.active = active;
}

/// Switch between a box and an upright capsule.
pub fn set_shape(o: &mut Obstacle, shape: ObstacleShape) {
    o.shape = shape;
}

/// Set the shape's centre, in local space. A non-finite centre is ignored.
pub fn set_center(o: &mut Obstacle, center: Vec3) {
    if center.is_finite() {
        o.center = center;
    }
}

/// Set the box's size, each side floored at [`MIN_SIZE`]. Non-finite is ignored.
pub fn set_size(o: &mut Obstacle, size: Vec3) {
    if size.is_finite() {
        o.size = size.max(Vec3::splat(MIN_SIZE));
    }
}

/// Set the capsule's radius, floored at [`MIN_SIZE`].
pub fn set_radius(o: &mut Obstacle, radius: f32) {
    o.radius = at_least(radius, MIN_SIZE);
}

/// Set the capsule's full height, floored at [`MIN_SIZE`].
pub fn set_height(o: &mut Obstacle, height: f32) {
    o.height = at_least(height, MIN_SIZE);
}

/// Turn carving on or off (Unity's `carving`).
pub fn set_carve(o: &mut Obstacle, carve: bool) {
    o.carve = carve;
}

/// Carve only once stationary (Unity's `carveOnlyStationary`).
pub fn set_carve_only_stationary(o: &mut Obstacle, only: bool) {
    o.carve_only_stationary = only;
}

/// Set the distance that counts as moving, floored at 0.
pub fn set_move_threshold(o: &mut Obstacle, metres: f32) {
    o.move_threshold = at_least(metres, 0.0);
}

/// Set the seconds of standing still before carving, floored at 0.
pub fn set_time_to_stationary(o: &mut Obstacle, seconds: f32) {
    o.time_to_stationary = at_least(seconds, 0.0);
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
        let mut o = Obstacle::default();
        set_size(&mut o, Vec3::new(2.0, -1.0, 0.0));
        assert_eq!(o.size, Vec3::new(2.0, MIN_SIZE, MIN_SIZE));
        set_size(&mut o, Vec3::NAN);
        assert_eq!(o.size.x, 2.0, "a non-finite size is ignored");
        set_radius(&mut o, f32::NAN);
        assert_eq!(o.radius, MIN_SIZE);
        set_height(&mut o, -3.0);
        assert_eq!(o.height, MIN_SIZE);
        set_move_threshold(&mut o, -0.5);
        assert_eq!(o.move_threshold, 0.0);
        set_time_to_stationary(&mut o, 2.0);
        assert_eq!(o.time_to_stationary, 2.0);
        set_center(&mut o, Vec3::Y);
        set_center(&mut o, Vec3::INFINITY);
        assert_eq!(o.center, Vec3::Y);
        set_shape(&mut o, ObstacleShape::Capsule);
        set_carve(&mut o, true);
        set_carve_only_stationary(&mut o, false);
        set_active(&mut o, false);
        assert_eq!(
            (o.shape, o.carve, o.carve_only_stationary, o.active),
            (ObstacleShape::Capsule, true, false, false)
        );
    }
}
