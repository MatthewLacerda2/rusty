use glam::{Vec2, Vec3};

use rusty_scene::camera::DEFAULT_FOV;
pub use rusty_scene::camera::MAX_PITCH;
use rusty_scene::Camera;
/// The nearest the eye comes to the pivot, in world units; zooming in past it pushes the
/// pivot ahead.
pub const MIN_DISTANCE: f32 = 0.05;
pub const MAX_DISTANCE: f32 = 5000.0;
pub const DEFAULT_DISTANCE: f32 = 10.0;
pub const DEFAULT_FLY_SPEED: f32 = 10.0;
/// Fly-speed range the scroll wheel moves within while flying.
pub const FLY_SPEED_RANGE: (f32, f32) = (0.5, 200.0);
/// Shift multiplies the fly speed by this.
pub const FAST_MULTIPLIER: f32 = 3.0;
const DEGREES_PER_POINT: f32 = 0.25;
/// Fraction of the pivot distance one scroll step zooms.
pub const ZOOM_PER_STEP: f32 = 0.15;
pub const POINTS_PER_STEP: f32 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCamera {
    pub pivot: Vec3,
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

    pub fn forward(&self) -> Vec3 {
        Camera::new(Vec3::ZERO, self.yaw, self.pitch).forward()
    }

    pub fn right(&self) -> Vec3 {
        Camera::new(Vec3::ZERO, self.yaw, self.pitch).right()
    }

    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    pub fn position(&self) -> Vec3 {
        self.pivot - self.forward() * self.distance
    }

    pub fn camera(&self) -> Camera {
        Camera::new(self.position(), self.yaw, self.pitch)
    }

    pub fn look(&mut self, delta: Vec2) {
        let eye = self.position();
        self.turn(delta);
        self.pivot = eye + self.forward() * self.distance;
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.turn(delta);
    }

    pub fn pan(&mut self, delta: Vec2, viewport_height: f32) {
        let world_per_point = self.world_per_point(viewport_height);
        self.pivot += (-self.right() * delta.x + self.up() * delta.y) * world_per_point;
    }

    pub fn zoom(&mut self, steps: f32) {
        let wanted = self.distance * (1.0 - ZOOM_PER_STEP).powf(steps);
        if wanted < MIN_DISTANCE {
            self.pivot += self.forward() * (self.distance - wanted).max(0.0);
            self.distance = MIN_DISTANCE;
        } else {
            self.distance = wanted.min(MAX_DISTANCE);
        }
    }

    pub fn fly(&mut self, axes: Vec3, fast: bool, dt: f32) {
        let dir = self.right() * axes.x + Vec3::Y * axes.y + self.forward() * axes.z;
        let Some(dir) = dir.try_normalize() else {
            return;
        };
        let speed = self.fly_speed * if fast { FAST_MULTIPLIER } else { 1.0 };
        self.pivot += dir * speed * dt;
    }

    pub fn adjust_fly_speed(&mut self, steps: f32) {
        let (lo, hi) = FLY_SPEED_RANGE;
        self.fly_speed = (self.fly_speed * 1.2f32.powf(steps)).clamp(lo, hi);
    }

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

    fn turn(&mut self, delta: Vec2) {
        self.yaw += delta.x * DEGREES_PER_POINT;
        self.pitch = (self.pitch - delta.y * DEGREES_PER_POINT).clamp(-MAX_PITCH, MAX_PITCH);
    }
}

#[cfg(test)]
#[path = "scene_camera_tests.rs"]
mod scene_camera_tests;
