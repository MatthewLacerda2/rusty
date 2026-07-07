//! src/scene/authoring/camera.rs — Shared camera-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `CameraComponent` field by field: the projection (`fov` / `near` / `far`), the
//! `culling_mask`, the camera-stack knobs (`render_order` / `clear_flags`), and the
//! motion-blur fields.
//!
//! The editor's Camera card routes every field write through these (#287). The Lua
//! `Camera.*` namespace drives the *render* `Camera` (yaw/pitch/position/fov of the
//! viewport), NOT this per-entity component, so it shares no field with this card.
//! The component's motion-blur fields ARE also written from `Graphics.*`
//! (`SetMotionBlurActive` / `SetMotionBlurSamples`); those route through
//! [`set_motion_blur_active`] / [`set_motion_blur_samples`] here, so the card and the
//! `Graphics` binding share one write. The `Graphics` sample clamp (`2..=32`) stays in
//! that adapter — it is an API-input bound the editor card deliberately does not apply
//! (the card forces a fixed 64-sample high-quality mode), so the ops are plain sets to
//! keep both behaviours byte-identical.
//!
//! Allowed deps: components (the `CameraComponent`/`ClearFlags` data). Pure.

use crate::components::{ClearFlags, CameraComponent};

/// Set the camera's field of view (degrees).
pub fn set_fov(c: &mut CameraComponent, fov: f32) {
    c.fov = fov;
}

/// Set the camera's near clip plane.
pub fn set_near(c: &mut CameraComponent, near: f32) {
    c.near = near;
}

/// Set the camera's far clip plane.
pub fn set_far(c: &mut CameraComponent, far: f32) {
    c.far = far;
}

/// Set the camera's layer culling mask (one bit per layer).
pub fn set_culling_mask(c: &mut CameraComponent, mask: u32) {
    c.culling_mask = mask;
}

/// Set the camera's stacking order (Unity "Depth").
pub fn set_render_order(c: &mut CameraComponent, render_order: i32) {
    c.render_order = render_order;
}

/// Set the camera's clear flags (how it initializes the framebuffer).
pub fn set_clear_flags(c: &mut CameraComponent, clear_flags: ClearFlags) {
    c.clear_flags = clear_flags;
}

/// Set the camera's motion-blur active flag.
pub fn set_motion_blur_active(c: &mut CameraComponent, active: bool) {
    c.motion_blur_active = active;
}

/// Set the camera's motion-blur sample count (no clamp — callers bound their input).
pub fn set_motion_blur_samples(c: &mut CameraComponent, samples: u32) {
    c.motion_blur_samples = samples;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::CameraComponent;
    use crate::scene::Scene;

    fn scene_with_camera() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Cam".to_string());
        let c = CameraComponent {
            active: true,
            fov: 60.0,
            near: 0.1,
            far: 1000.0,
            culling_mask: u32::MAX,
            render_order: 0,
            clear_flags: ClearFlags::Skybox,
            motion_blur_active: false,
            motion_blur_samples: 0,
        };
        scene.world.set_camera(id, Some(c));
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_camera();
        let mut e = scene.world.camera_mut(id).unwrap();
        set_fov(&mut e, 90.0);
        set_near(&mut e, 0.5);
        set_far(&mut e, 500.0);
        set_culling_mask(&mut e, 0b1010);
        set_render_order(&mut e, 3);
        set_clear_flags(&mut e, ClearFlags::DepthOnly);
        set_motion_blur_active(&mut e, true);
        set_motion_blur_samples(&mut e, 64);
        let c = &*e;
        assert_eq!(c.fov, 90.0);
        assert_eq!(c.near, 0.5);
        assert_eq!(c.far, 500.0);
        assert_eq!(c.culling_mask, 0b1010);
        assert_eq!(c.render_order, 3);
        assert_eq!(c.clear_flags, ClearFlags::DepthOnly);
        assert!(c.motion_blur_active);
        assert_eq!(c.motion_blur_samples, 64);
    }
}
