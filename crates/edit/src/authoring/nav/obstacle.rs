use glam::Vec3;

use rusty_components::{NavMeshObstacleComponent as Obstacle, ObstacleShape};
use rusty_scene::rules::sanitize::{at_least, finite3_or, non_negative, MIN_EXTENT};

pub const MIN_SIZE: f32 = MIN_EXTENT;

/// Enable or disable the obstacle (Unity's `enabled`).
pub fn set_active(o: &mut Obstacle, active: bool) {
    o.active = active;
}

pub fn set_shape(o: &mut Obstacle, shape: ObstacleShape) {
    o.shape = shape;
}

pub fn set_center(o: &mut Obstacle, center: Vec3) {
    o.center = finite3_or(center, o.center);
}

pub fn set_size(o: &mut Obstacle, size: Vec3) {
    o.size = finite3_or(size, o.size).max(Vec3::splat(MIN_SIZE));
}

pub fn set_radius(o: &mut Obstacle, radius: f32) {
    o.radius = at_least(radius, MIN_SIZE);
}

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

pub fn set_move_threshold(o: &mut Obstacle, metres: f32) {
    o.move_threshold = non_negative(metres);
}

/// Set the seconds of standing still before carving, floored at 0.
pub fn set_time_to_stationary(o: &mut Obstacle, seconds: f32) {
    o.time_to_stationary = non_negative(seconds);
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
        set_time_to_stationary(&mut o, f32::INFINITY);
        assert_eq!(o.time_to_stationary, 0.0, "+inf is refused like NaN");
        set_radius(&mut o, f32::INFINITY);
        assert_eq!(o.radius, MIN_SIZE);
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
