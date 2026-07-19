//! src/editor/inspector/preview/orbit.rs — the Preview tab's orbit camera (#352).
//!
//! Pure math + state, no egui/GPU types, so it's unit-testable without a window.
//! Mirrors a standard asset-preview orbit rig: drag rotates around a fixed target,
//! wheel zooms distance in/out. The camera always looks at the target — there is no
//! pan, matching the issue's "viewer, not an editor" scope.

use glam::Vec3;

use crate::render::Camera;

/// Degrees per point of horizontal/vertical drag.
const DRAG_SENSITIVITY: f32 = 0.4;
/// Distance change per unit of scroll delta; clamped to keep the subject in frame.
const ZOOM_SENSITIVITY: f32 = 0.01;
const MIN_DISTANCE: f32 = 0.5;
const MAX_DISTANCE: f32 = 20.0;
const MAX_PITCH: f32 = 89.0;

/// Orbit-camera state for the Preview tab, persisted on [`crate::editor::EditorUi`]
/// across frames so drag/zoom accumulate. Always orbits the world origin — the
/// preview mesh is always placed there (see `scene::build_preview_scene`).
#[derive(Clone, Copy, Debug)]
pub struct OrbitState {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for OrbitState {
    fn default() -> Self {
        // A pleasant default 3/4 view, matching the material-preview convention of
        // every reference engine (slightly above and to the side).
        Self {
            yaw: -30.0,
            pitch: -20.0,
            distance: 3.0,
        }
    }
}

impl OrbitState {
    /// Apply one frame's pointer drag (points) and scroll delta to the orbit angles
    /// and distance, clamping pitch away from the poles and distance to a sane
    /// preview range.
    pub fn apply_input(&mut self, drag_delta: egui::Vec2, scroll_delta: f32) {
        self.yaw -= drag_delta.x * DRAG_SENSITIVITY;
        self.pitch = (self.pitch + drag_delta.y * DRAG_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_rotates_yaw_and_clamped_pitch() {
        let mut orbit = OrbitState::default();
        orbit.apply_input(egui::vec2(100.0, 1000.0), 0.0);
        assert!(orbit.yaw < OrbitState::default().yaw);
        assert_eq!(orbit.pitch, MAX_PITCH);
    }

    #[test]
    fn scroll_zooms_within_bounds() {
        let mut orbit = OrbitState {
            distance: 1.0,
            ..OrbitState::default()
        };
        orbit.apply_input(egui::Vec2::ZERO, -100_000.0);
        assert_eq!(orbit.distance, MAX_DISTANCE);
        orbit.apply_input(egui::Vec2::ZERO, 100_000.0);
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
}
