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

use crate::components::{ClearFlags, Entity};

/// Set the camera's field of view (degrees).
pub fn set_fov(entity: &mut Entity, fov: f32) {
    if let Some(c) = &mut entity.camera {
        c.fov = fov;
    }
}

/// Set the camera's near clip plane.
pub fn set_near(entity: &mut Entity, near: f32) {
    if let Some(c) = &mut entity.camera {
        c.near = near;
    }
}

/// Set the camera's far clip plane.
pub fn set_far(entity: &mut Entity, far: f32) {
    if let Some(c) = &mut entity.camera {
        c.far = far;
    }
}

/// Set the camera's layer culling mask (one bit per layer).
pub fn set_culling_mask(entity: &mut Entity, mask: u32) {
    if let Some(c) = &mut entity.camera {
        c.culling_mask = mask;
    }
}

/// Set the camera's stacking order (Unity "Depth").
pub fn set_render_order(entity: &mut Entity, render_order: i32) {
    if let Some(c) = &mut entity.camera {
        c.render_order = render_order;
    }
}

/// Set the camera's clear flags (how it initializes the framebuffer).
pub fn set_clear_flags(entity: &mut Entity, clear_flags: ClearFlags) {
    if let Some(c) = &mut entity.camera {
        c.clear_flags = clear_flags;
    }
}

/// Set the camera's motion-blur active flag.
pub fn set_motion_blur_active(entity: &mut Entity, active: bool) {
    if let Some(c) = &mut entity.camera {
        c.motion_blur_active = active;
    }
}

/// Set the camera's motion-blur sample count (no clamp — callers bound their input).
pub fn set_motion_blur_samples(entity: &mut Entity, samples: u32) {
    if let Some(c) = &mut entity.camera {
        c.motion_blur_samples = samples;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::CameraComponent;
    use crate::scene::Scene;

    fn scene_with_camera() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Cam".to_string());
        if let Some(mut e) = scene.get_entity_mut(id) {
            e.camera = Some(CameraComponent {
                active: true,
                fov: 60.0,
                near: 0.1,
                far: 1000.0,
                culling_mask: u32::MAX,
                render_order: 0,
                clear_flags: ClearFlags::Skybox,
                motion_blur_active: false,
                motion_blur_samples: 0,
            });
        }
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_camera();
        let mut e = scene.get_entity_mut(id).unwrap();
        set_fov(&mut e, 90.0);
        set_near(&mut e, 0.5);
        set_far(&mut e, 500.0);
        set_culling_mask(&mut e, 0b1010);
        set_render_order(&mut e, 3);
        set_clear_flags(&mut e, ClearFlags::DepthOnly);
        set_motion_blur_active(&mut e, true);
        set_motion_blur_samples(&mut e, 64);
        let c = e.camera.as_ref().unwrap();
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
