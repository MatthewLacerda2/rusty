use glam::Vec3;

use rusty_scene::camera::MAX_PITCH;
use rusty_scene::Camera;

/// Degrees per point of horizontal/vertical drag.
const DRAG_SENSITIVITY: f32 = 0.4;
/// Distance change per unit of scroll delta; clamped to keep the subject in frame.
const ZOOM_SENSITIVITY: f32 = 0.01;
const MIN_DISTANCE: f32 = 0.5;
const MAX_DISTANCE: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitState {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for OrbitState {
    fn default() -> Self {
        Self {
            yaw: -30.0,
            pitch: -20.0,
            distance: 3.0,
        }
    }
}

impl OrbitState {
    pub fn apply_input(&mut self, drag_x: f32, drag_y: f32, scroll_delta: f32) {
        self.yaw -= drag_x * DRAG_SENSITIVITY;
        self.pitch = (self.pitch + drag_y * DRAG_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
        self.distance =
            (self.distance - scroll_delta * ZOOM_SENSITIVITY).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Build this frame's [`Camera`], orbiting the world origin at `self.distance`.
    pub fn camera(&self) -> Camera {
        let mut camera = Camera::new(Vec3::ZERO, self.yaw, self.pitch);
        camera.position = -camera.forward() * self.distance;
        camera
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramedOrbit {
    /// The orbit the tab draws, moved by drag and wheel.
    pub orbit: OrbitState,
    framing: OrbitState,
}

impl FramedOrbit {
    pub fn frame(&mut self, framing: OrbitState) {
        if self.framing != framing {
            *self = Self {
                orbit: framing,
                framing,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_rotates_yaw_and_clamped_pitch() {
        let mut orbit = OrbitState::default();
        orbit.apply_input(100.0, 1000.0, 0.0);
        assert!(orbit.yaw < OrbitState::default().yaw);
        assert_eq!(orbit.pitch, MAX_PITCH);
    }

    #[test]
    fn scroll_zooms_within_bounds() {
        let mut orbit = OrbitState {
            distance: 1.0,
            ..OrbitState::default()
        };
        orbit.apply_input(0.0, 0.0, -100_000.0);
        assert_eq!(orbit.distance, MAX_DISTANCE);
        orbit.apply_input(0.0, 0.0, 100_000.0);
        assert_eq!(orbit.distance, MIN_DISTANCE);
    }

    #[test]
    fn camera_always_looks_at_the_origin() {
        let orbit = OrbitState::default();
        let camera = orbit.camera();
        // position + forward*distance should land back on the origin.
        let recomposed = camera.position + camera.forward() * orbit.distance;
        assert!(recomposed.length() < 1e-4);
    }

    #[test]
    fn a_framed_orbit_starts_over_only_when_its_framing_changes() {
        let low = OrbitState {
            yaw: -90.0,
            pitch: -25.0,
            distance: 2.0,
        };
        let mut framed = FramedOrbit::default();
        framed.orbit.apply_input(50.0, 0.0, 0.0);
        let tuned = framed.orbit;
        framed.frame(OrbitState::default());
        assert_eq!(
            framed.orbit, tuned,
            "the same framing keeps a hand-tuned orbit"
        );
        framed.frame(low);
        assert_eq!(framed.orbit, low, "a new framing starts the orbit there");
        framed.frame(OrbitState::default());
        assert_eq!(framed.orbit, OrbitState::default(), "and back");
    }

    #[test]
    fn default_framing_is_stable_so_headless_shots_are_comparable() {
        // `Debug.Preview` renders a fixed framing every time: two
        // captures of the same asset must agree on the camera exactly.
        let a = OrbitState::default().camera();
        let b = OrbitState::default().camera();
        assert_eq!(a.position, b.position);
        assert_eq!((a.yaw, a.pitch), (b.yaw, b.pitch));
    }
}
