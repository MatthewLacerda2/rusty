//! src/scene/authoring/rendering/camera.rs — Shared camera-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `CameraComponent` field by field: the projection (`fov` / `near` / `far`), the
//! `culling_mask`, the camera-stack knobs (`render_order` / `clear_flags`), and the
//! motion-blur fields, the projection and the render-texture target (#430).
//!
//! The editor's Camera card routes every field write through these (#287). The Lua
//! `Camera.*` namespace mostly drives the *render* `Camera` (yaw/pitch/position/fov
//! of the viewport); its per-entity functions (projection, culling mask, render
//! target, lens and stacking — #430, #827, #1341) take an id and write this
//! component through these ops. The lens ops hold Unity's bounds (FOV 1–179°,
//! near above 0, far beyond near) on every write, so neither the card nor a script
//! can write a broken frustum. A value that skips them (a scene file, a prefab) is
//! held to them where the view camera copies its lens (`scene::camera::held_lens`,
//! #1359).
//! The component's motion-blur fields ARE also written from `Graphics.*`
//! (`SetMotionBlurActive` / `Samples` / `Intensity` / `MaxLength`); those route
//! through the `set_motion_blur_*` ops here, so the card and the `Graphics` binding
//! share one write. Each clamp (samples `2..=32`, intensity `0..=1`, clamp
//! `0..=0.2`) lives in its op, so the card, scripts and the renderer agree.
//!
//! Pure.

use crate::components::{CameraComponent, ClearFlags, Projection, RenderTarget};
use crate::scene::authoring::sanitize::{at_least, saturated};

/// The field-of-view range, in degrees: Unity's Camera inspector bounds, applied
/// to every write (the view camera's `Camera.SetFov` too).
pub const MIN_FOV: f32 = 1.0;
pub const MAX_FOV: f32 = 179.0;
/// The nearest a near plane may sit (Unity's inspector floor): a viewmodel's
/// 0.01 is allowed, 0 (an infinite depth range) is not.
pub const MIN_NEAR: f32 = 0.01;
/// How far beyond the near plane the far plane must at least sit (Unity's gap).
pub const MIN_CLIP_GAP: f32 = 0.01;
/// The smallest orthographic half-height, so the view keeps a volume.
pub const MIN_ORTHO_SIZE: f32 = 0.01;

/// Set the camera's vertical field of view (degrees), clamped to
/// [`MIN_FOV`]..=[`MAX_FOV`]. A NaN takes [`MIN_FOV`] (`f32::clamp` would keep it).
pub fn set_fov(c: &mut CameraComponent, fov: f32) {
    c.fov = saturated(fov, MIN_FOV, MAX_FOV);
}

/// Set both clip planes the way Unity holds them: `near` at least [`MIN_NEAR`],
/// `far` at least [`MIN_CLIP_GAP`] beyond it (a near plane past the far one pushes
/// the far one out). A non-finite plane takes its lower bound, and the far plane
/// always sits strictly beyond the near one, even where `near + MIN_CLIP_GAP`
/// rounds back to `near`.
pub fn set_clip_planes(c: &mut CameraComponent, near: f32, far: f32) {
    // Below f32::MAX, so the plane just beyond it is still finite.
    let near = at_least(near, MIN_NEAR).min(f32::MAX.next_down());
    let min_far = (near + MIN_CLIP_GAP).max(near.next_up());
    c.near = near;
    c.far = at_least(far, min_far);
}

/// Set the camera's near clip plane (see [`set_clip_planes`]).
pub fn set_near(c: &mut CameraComponent, near: f32) {
    set_clip_planes(c, near, c.far);
}

/// Set the camera's far clip plane (see [`set_clip_planes`]).
pub fn set_far(c: &mut CameraComponent, far: f32) {
    set_clip_planes(c, c.near, far);
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

/// Set the camera's motion-blur sample count, clamped to the range the renderer
/// honours (`MIN_MOTION_BLUR_SAMPLES..=MAX_MOTION_BLUR_SAMPLES`).
pub fn set_motion_blur_samples(c: &mut CameraComponent, samples: u32) {
    c.motion_blur_samples = samples.clamp(
        CameraComponent::MIN_MOTION_BLUR_SAMPLES,
        CameraComponent::MAX_MOTION_BLUR_SAMPLES,
    );
}

/// Set the camera's motion-blur intensity (#1436, the shutter fraction), clamped to
/// `0..=MAX_MOTION_BLUR_INTENSITY`. A NaN takes 0.
pub fn set_motion_blur_intensity(c: &mut CameraComponent, intensity: f32) {
    c.motion_blur_intensity = saturated(intensity, 0.0, CameraComponent::MAX_MOTION_BLUR_INTENSITY);
}

/// Set the camera's longest motion-blur streak (#1436, a fraction of the view's
/// height), clamped to `0..=MAX_MOTION_BLUR_MAX_LENGTH`. A NaN takes 0.
pub fn set_motion_blur_max_length(c: &mut CameraComponent, max_length: f32) {
    c.motion_blur_max_length =
        saturated(max_length, 0.0, CameraComponent::MAX_MOTION_BLUR_MAX_LENGTH);
}

/// Set the camera's FXAA flag — the anti-aliasing pass at the end of the post-FX
/// chain (#360). Shared by the editor's Camera card and `Graphics.SetFxaaActive`.
pub fn set_fxaa_active(c: &mut CameraComponent, active: bool) {
    c.fxaa_active = active;
}

/// The largest render-texture side, in pixels — a guard against a typo allocating
/// gigabytes, not a quality bound (4096² is a 64 MB colour target).
pub const MAX_TARGET_SIZE: u32 = 4096;

/// Set the camera's projection (#430). An orthographic size is at least
/// [`MIN_ORTHO_SIZE`].
// lint: allow(sanitize) the orthographic size, its one float, goes through `at_least`
pub fn set_projection(c: &mut CameraComponent, projection: Projection) {
    c.projection = match projection {
        Projection::Orthographic { size } => Projection::Orthographic {
            size: at_least(size, MIN_ORTHO_SIZE),
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
            fov: 60.0,
            far: 1000.0,
            ..CameraComponent::default()
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
        set_motion_blur_samples(&mut e, 16);
        set_motion_blur_intensity(&mut e, 0.75);
        set_motion_blur_max_length(&mut e, 0.1);
        set_fxaa_active(&mut e, false);
        let c = &*e;
        assert_eq!(c.fov, 90.0);
        assert_eq!(c.near, 0.5);
        assert_eq!(c.far, 500.0);
        assert_eq!(c.culling_mask, 0b1010);
        assert_eq!(c.render_order, 3);
        assert_eq!(c.clear_flags, ClearFlags::DepthOnly);
        assert!(c.motion_blur_active);
        assert_eq!(c.motion_blur_samples, 16);
        assert_eq!(c.motion_blur_intensity, 0.75);
        assert_eq!(c.motion_blur_max_length, 0.1);
        assert!(!c.fxaa_active, "the op must be able to turn FXAA off");
    }

    #[test]
    fn motion_blur_samples_clamp_to_the_renderer_range() {
        let (mut scene, id) = scene_with_camera();
        let mut e = scene.world.camera_mut(id).unwrap();
        set_motion_blur_samples(&mut e, 64);
        assert_eq!(e.motion_blur_samples, 32);
        set_motion_blur_samples(&mut e, 0);
        assert_eq!(e.motion_blur_samples, 2);
    }

    #[test]
    fn motion_blur_intensity_and_clamp_hold_their_ranges() {
        let (mut scene, id) = scene_with_camera();
        let mut e = scene.world.camera_mut(id).unwrap();
        set_motion_blur_intensity(&mut e, 3.0);
        set_motion_blur_max_length(&mut e, 1.0);
        assert_eq!(
            (e.motion_blur_intensity, e.motion_blur_max_length),
            (1.0, 0.2)
        );
        set_motion_blur_intensity(&mut e, f32::NAN);
        set_motion_blur_max_length(&mut e, -1.0);
        assert_eq!(
            (e.motion_blur_intensity, e.motion_blur_max_length),
            (0.0, 0.0)
        );
    }
}
