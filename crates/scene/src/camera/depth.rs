//! Depth runs reversed, as in Unity: near at [`NEAR`] (1), far at [`FAR`] (0). A float's
//! precision is densest near 0, where perspective spends it least, so a far plane of kilometres
//! does not z-fight. The renderer (`render::gpu::depth`) and the shaders (`depth.wgsl`) say the
//! same; a test holds them equal. Shadow maps keep ordinary depth.

use glam::camera::rh::proj::directx;
use glam::Mat4;

/// The near plane's depth.
pub const NEAR: f32 = 1.0;
/// The far plane's depth: what a view's depth target is cleared to.
pub const FAR: f32 = 0.0;

/// Whether depth `a` is nearer the eye than `b`.
pub fn in_front(a: f32, b: f32) -> bool {
    a > b
}

/// Right-handed, view distance `near` to [`NEAR`] and `far` to [`FAR`].
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    directx::perspective(fov_y, aspect, far, near)
}

/// Right-handed, reversed as [`perspective`].
pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    directx::orthographic(left, right, bottom, top, far, near)
}

#[cfg(test)]
#[path = "depth_tests.rs"]
mod depth_tests;
