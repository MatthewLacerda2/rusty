//! The lens ops hold Unity's bounds (FOV 1–179°, near above 0, far beyond near); a value that
//! skips them is held to them in `scene::camera::held_lens`.

use rusty_components::{CameraComponent, ClearFlags, FovAxis, Projection, RenderTarget};
use rusty_core::settings::quality::EffectQuality;
use rusty_scene::rules::lens;
use rusty_scene::rules::sanitize::saturated;

pub use rusty_scene::rules::taa::{
    camera_knob, set_taa_active, set_taa_anti_flicker, set_taa_base_blend_factor,
    set_taa_contrast_adaptive_sharpening, set_taa_jitter_scale, set_taa_mip_bias, set_taa_quality,
    set_taa_variance_clamp_scale, CameraKnob, TAA_EFFECT, TAA_KNOBS,
};

pub use rusty_scene::rules::lens::{MAX_FOV, MIN_CLIP_GAP, MIN_FOV, MIN_NEAR, MIN_ORTHO_SIZE};

pub fn set_fov(c: &mut CameraComponent, fov: f32) {
    c.fov = lens::hold_fov(fov);
}

/// Display only, as Unity's *FOV Axis*.
pub fn set_fov_axis(c: &mut CameraComponent, axis: FovAxis) {
    c.fov_axis = axis;
}

/// As Unity holds them: a near plane past the far one pushes the far one strictly beyond it, even
/// where `near + MIN_CLIP_GAP` rounds back to `near`.
pub fn set_clip_planes(c: &mut CameraComponent, near: f32, far: f32) {
    (c.near, c.far) = lens::hold_clip_planes(near, far);
}

pub fn set_near(c: &mut CameraComponent, near: f32) {
    set_clip_planes(c, near, c.far);
}

pub fn set_far(c: &mut CameraComponent, far: f32) {
    set_clip_planes(c, c.near, far);
}

pub fn set_culling_mask(c: &mut CameraComponent, mask: u32) {
    c.culling_mask = mask;
}

/// Unity's "Depth".
pub fn set_render_order(c: &mut CameraComponent, render_order: i32) {
    c.render_order = render_order;
}

pub fn set_clear_flags(c: &mut CameraComponent, clear_flags: ClearFlags) {
    c.clear_flags = clear_flags;
}

pub fn set_motion_blur_active(c: &mut CameraComponent, active: bool) {
    c.motion_blur_active = active;
}

pub fn set_motion_blur_quality(c: &mut CameraComponent, quality: EffectQuality) {
    c.motion_blur_quality = quality;
}

pub fn set_motion_blur_samples(c: &mut CameraComponent, samples: u32) {
    c.motion_blur_samples = samples.clamp(
        CameraComponent::MIN_MOTION_BLUR_SAMPLES,
        CameraComponent::MAX_MOTION_BLUR_SAMPLES,
    );
}

/// The shutter fraction; a NaN takes 0.
pub fn set_motion_blur_intensity(c: &mut CameraComponent, intensity: f32) {
    c.motion_blur_intensity = saturated(intensity, 0.0, CameraComponent::MAX_MOTION_BLUR_INTENSITY);
}

/// A fraction of the view's height; a NaN takes 0.
pub fn set_motion_blur_max_length(c: &mut CameraComponent, max_length: f32) {
    c.motion_blur_max_length =
        saturated(max_length, 0.0, CameraComponent::MAX_MOTION_BLUR_MAX_LENGTH);
}

pub fn set_fxaa_active(c: &mut CameraComponent, active: bool) {
    c.fxaa_active = active;
}

/// Unity's `Camera.useOcclusionCulling`.
pub fn set_use_occlusion_culling(c: &mut CameraComponent, on: bool) {
    c.use_occlusion_culling = on;
}

/// Pixels per side: a guard against a typo allocating gigabytes, not a quality bound (4096² is a
/// 64 MB colour target).
pub const MAX_TARGET_SIZE: u32 = 4096;

pub fn set_projection(c: &mut CameraComponent, projection: Projection) {
    c.projection = lens::hold_projection(projection);
}

pub fn projection_name(p: Projection) -> &'static str {
    match p {
        Projection::Perspective => "Perspective",
        Projection::Orthographic { .. } => "Orthographic",
    }
}

pub fn parse_projection(name: &str, size: f32) -> Option<Projection> {
    match name.to_ascii_lowercase().as_str() {
        "perspective" => Some(Projection::Perspective),
        "orthographic" => Some(Projection::Orthographic { size }),
        _ => None,
    }
}

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

pub fn set_target_post_fx(c: &mut CameraComponent, on: bool) {
    if let Some(t) = c.target_texture.as_mut() {
        t.post_fx = on;
    }
}

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
    use rusty_components::CameraComponent;
    use rusty_scene::Scene;

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
