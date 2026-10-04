//! src/editor/viewport/scene_camera.rs — the Scene view's own camera (#745).
//!
//! Unity's Scene camera is an editor-only pose, independent of every scene Camera
//! and of Play: the Scene tab renders from it, the Game tab and the sim never see it.
//! Like Unity's `SceneView`, the pose is a **pivot** the camera looks at, a
//! **distance** back from it and a yaw/pitch, so each gesture is one small edit:
//! orbit turns around the pivot, look turns in place (the pivot swings with the
//! view), pan slides the pivot across the screen, zoom changes the distance, fly
//! moves pivot and eye together, and frame-selected sets the pivot to the selection.
//! The pose resets per editor session; it is not saved with the scene.

use glam::{Vec2, Vec3};

use crate::scene::camera::DEFAULT_FOV;
use crate::scene::Camera;

/// Pitch limit, short of straight up/down where yaw stops meaning anything.
pub const MAX_PITCH: f32 = 89.0;
/// The closest the eye gets to the pivot; zooming in past it pushes the pivot ahead.
pub const MIN_DISTANCE: f32 = 0.05;
/// The farthest zoom-out.
pub const MAX_DISTANCE: f32 = 5000.0;
/// Pivot distance the first pose assumes: what an editor's default framing looks at.
pub const DEFAULT_DISTANCE: f32 = 10.0;
/// Fly speed (world units per second) the session starts with — the old WASD speed.
pub const DEFAULT_FLY_SPEED: f32 = 10.0;
/// Fly-speed range the scroll wheel moves within while flying.
pub const FLY_SPEED_RANGE: (f32, f32) = (0.5, 200.0);
/// Shift multiplies the fly speed by this.
pub const FAST_MULTIPLIER: f32 = 3.0;
/// Degrees of turn per point (or raw mouse count) of pointer motion.
pub const DEGREES_PER_POINT: f32 = 0.25;
/// Fraction of the pivot distance one scroll step (one notch, 50 points) zooms.
pub const ZOOM_PER_STEP: f32 = 0.15;
/// Points of scroll that make one step, egui's line height times the notch lines.
pub const POINTS_PER_STEP: f32 = 50.0;

/// The Scene view's camera pose. See the module docs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCamera {
    /// The point the camera looks at and orbits around.
    pub pivot: Vec3,
    /// How far the eye sits back from the pivot along the view direction.
    pub distance: f32,
    /// Degrees around +Y, [`Camera`]'s convention.
    pub yaw: f32,
    /// Degrees up from the horizon, clamped to ±[`MAX_PITCH`].
    pub pitch: f32,
    /// Fly speed in world units per second (Shift multiplies it).
    pub fly_speed: f32,
}

impl Default for SceneCamera {
    /// The engine's default edit-mode framing (`GameWorld::new`'s camera).
    fn default() -> Self {
        Self::from_camera(&Camera::new(Vec3::new(0.0, 5.0, -10.0), 90.0, -20.0))
    }
}

impl SceneCamera {
    /// A pose that sees exactly what `camera` sees, pivoting [`DEFAULT_DISTANCE`] ahead.
    pub fn from_camera(camera: &Camera) -> Self {
        let pitch = camera.pitch.clamp(-MAX_PITCH, MAX_PITCH);
        let mut pose = Self {
            pivot: Vec3::ZERO,
            distance: DEFAULT_DISTANCE,
            yaw: camera.yaw,
            pitch,
            fly_speed: DEFAULT_FLY_SPEED,
        };
        pose.pivot = camera.position + pose.forward() * DEFAULT_DISTANCE;
        pose
    }

    /// The view direction.
    pub fn forward(&self) -> Vec3 {
        Camera::new(Vec3::ZERO, self.yaw, self.pitch).forward()
    }

    /// Screen-right in world space.
    pub fn right(&self) -> Vec3 {
        Camera::new(Vec3::ZERO, self.yaw, self.pitch).right()
    }

    /// Screen-up in world space.
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    /// Where the eye is.
    pub fn position(&self) -> Vec3 {
        self.pivot - self.forward() * self.distance
    }

    /// The render camera for this pose, with the editor's default lens.
    pub fn camera(&self) -> Camera {
        Camera::new(self.position(), self.yaw, self.pitch)
    }

    /// Mouse look (RMB): turn in place by `delta` points; the eye stays put.
    pub fn look(&mut self, delta: Vec2) {
        let eye = self.position();
        self.turn(delta);
        self.pivot = eye + self.forward() * self.distance;
    }

    /// Orbit (Alt + LMB): turn around the pivot by `delta` points; the pivot stays put.
    pub fn orbit(&mut self, delta: Vec2) {
        self.turn(delta);
    }

    /// Pan (MMB): slide the view so the scene follows the pointer `delta` points
    /// across a viewport `viewport_height` points tall, at the pivot's depth.
    pub fn pan(&mut self, delta: Vec2, viewport_height: f32) {
        let world_per_point = self.world_per_point(viewport_height);
        self.pivot += (-self.right() * delta.x + self.up() * delta.y) * world_per_point;
    }

    /// Zoom toward the pivot by `steps` scroll steps (positive = in). Each step takes
    /// a fixed fraction of the distance, so it feels the same near and far; at
    /// [`MIN_DISTANCE`] the pivot is pushed ahead instead, so zoom never sticks.
    pub fn zoom(&mut self, steps: f32) {
        let wanted = self.distance * (1.0 - ZOOM_PER_STEP).powf(steps);
        if wanted < MIN_DISTANCE {
            self.pivot += self.forward() * (self.distance - wanted).max(0.0);
            self.distance = MIN_DISTANCE;
        } else {
            self.distance = wanted.min(MAX_DISTANCE);
        }
    }

    /// Fly (RMB + WASD/QE): move eye and pivot together for `dt` seconds. `axes` is
    /// (right, up, forward) in -1..=1; up is world up, like Unity's Q/E.
    pub fn fly(&mut self, axes: Vec3, fast: bool, dt: f32) {
        let dir = self.right() * axes.x + Vec3::Y * axes.y + self.forward() * axes.z;
        let Some(dir) = dir.try_normalize() else {
            return;
        };
        let speed = self.fly_speed * if fast { FAST_MULTIPLIER } else { 1.0 };
        self.pivot += dir * speed * dt;
    }

    /// Scroll while flying: each step scales the fly speed by 20 %, within range.
    pub fn adjust_fly_speed(&mut self, steps: f32) {
        let (lo, hi) = FLY_SPEED_RANGE;
        self.fly_speed = (self.fly_speed * 1.2f32.powf(steps)).clamp(lo, hi);
    }

    /// Frame selected (F): pivot on `center` and back off until a sphere of
    /// `radius` fills the vertical field of view. The view direction is kept.
    pub fn frame(&mut self, center: Vec3, radius: f32) {
        let half_fov = (DEFAULT_FOV * 0.5).to_radians();
        self.pivot = center;
        self.distance = (radius.max(0.1) / half_fov.sin()).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// World units one point covers at the pivot's depth.
    fn world_per_point(&self, viewport_height: f32) -> f32 {
        let half_fov = (DEFAULT_FOV * 0.5).to_radians();
        2.0 * self.distance * half_fov.tan() / viewport_height.max(1.0)
    }

    /// Yaw right with +x, pitch up with -y (screen y grows down), clamped.
    fn turn(&mut self, delta: Vec2) {
        self.yaw += delta.x * DEGREES_PER_POINT;
        self.pitch = (self.pitch - delta.y * DEGREES_PER_POINT).clamp(-MAX_PITCH, MAX_PITCH);
    }
}

#[cfg(test)]
#[path = "scene_camera_tests.rs"]
mod scene_camera_tests;
