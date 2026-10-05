//! src/scene/authoring/camera.rs — Shared camera-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `CameraComponent` field by field: the projection (`fov` / `near` / `far`), the
//! `culling_mask`, the camera-stack knobs (`render_order` / `clear_flags`), and the
//! motion-blur fields, the projection and the render-texture target (#430).
//!
//! The editor's Camera card routes every field write through these (#287). The Lua
//! `Camera.*` namespace mostly drives the *render* `Camera` (yaw/pitch/position/fov
//! of the viewport); its per-entity functions (projection, culling mask, render
//! target — #430, #827) take an id and write this component through these ops.
//! The component's motion-blur fields ARE also written from `Graphics.*`
//! (`SetMotionBlurActive` / `SetMotionBlurSamples`); those route through
//! [`set_motion_blur_active`] / [`set_motion_blur_samples`] here, so the card and the
//! `Graphics` binding share one write. The `Graphics` sample clamp (`2..=32`) stays in
//! that adapter — it is an API-input bound the editor card deliberately does not apply
//! (the card forces a fixed 64-sample high-quality mode), so the ops are plain sets to
//! keep both behaviours byte-identical.
//!
//! Pure.

use crate::components::{CameraComponent, ClearFlags, Projection, RenderTarget};

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

/// Set the camera's FXAA flag — the anti-aliasing pass at the end of the post-FX
/// chain (#360). Shared by the editor's Camera card and `Graphics.SetFxaaActive`.
pub fn set_fxaa_active(c: &mut CameraComponent, active: bool) {
    c.fxaa_active = active;
}

/// The largest render-texture side, in pixels — a guard against a typo allocating
/// gigabytes, not a quality bound (4096² is a 64 MB colour target).
pub const MAX_TARGET_SIZE: u32 = 4096;

/// Set the camera's projection (#430). An orthographic size is clamped to > 0.
pub fn set_projection(c: &mut CameraComponent, projection: Projection) {
    c.projection = match projection {
        Projection::Orthographic { size } => Projection::Orthographic {
            size: size.max(0.01),
        },
        p => p,
    };
}

/// The projection's name: `"Perspective"` or `"Orthographic"`.
pub fn projection_name(p: Projection) -> &'static str {
    match p {
        Projection::Perspective => "Perspective",
        Projection::Orthographic { .. } => "Orthographic",
    }
}

/// Parse a projection name (case-insensitive) with the orthographic `size` to use.
pub fn parse_projection(name: &str, size: f32) -> Option<Projection> {
    match name.to_ascii_lowercase().as_str() {
        "perspective" => Some(Projection::Perspective),
        "orthographic" => Some(Projection::Orthographic { size }),
        _ => None,
    }
}

/// Point the camera at a render texture `name` at `width` x `height` (#430),
/// keeping its post-FX / update rate if it already had a target. An empty name
/// clears the target, returning the camera to the screen stack.
pub fn set_target_texture(c: &mut CameraComponent, name: &str, width: u32, height: u32) {
    let name = name.trim();
    if name.is_empty() {
        c.target_texture = None;
        return;
    }
    let (width, height) = (
        width.clamp(1, MAX_TARGET_SIZE),
        height.clamp(1, MAX_TARGET_SIZE),
    );
    let mut target = c
        .target_texture
        .take()
        .unwrap_or_else(|| RenderTarget::new(name, width, height));
    (target.name, target.width, target.height) = (name.to_string(), width, height);
    c.target_texture = Some(target);
}

/// Run the render texture's full post-FX chain or only tonemap it (no-op without a
/// target).
pub fn set_target_post_fx(c: &mut CameraComponent, on: bool) {
    if let Some(t) = c.target_texture.as_mut() {
        t.post_fx = on;
    }
}

/// Redraw the render texture every `n`th frame, at least 1 (no-op without a target).
pub fn set_target_update_every(c: &mut CameraComponent, n: u32) {
    if let Some(t) = c.target_texture.as_mut() {
        t.update_every = n.max(1);
    }
}

#[cfg(test)]
#[path = "camera_target_tests.rs"]
mod target_tests;

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
            fxaa_active: true,
            projection: Default::default(),
            target_texture: None,
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
        set_fxaa_active(&mut e, false);
        let c = &*e;
        assert_eq!(c.fov, 90.0);
        assert_eq!(c.near, 0.5);
        assert_eq!(c.far, 500.0);
        assert_eq!(c.culling_mask, 0b1010);
        assert_eq!(c.render_order, 3);
        assert_eq!(c.clear_flags, ClearFlags::DepthOnly);
        assert!(c.motion_blur_active);
        assert_eq!(c.motion_blur_samples, 64);
        assert!(!c.fxaa_active, "the op must be able to turn FXAA off");
    }
}
