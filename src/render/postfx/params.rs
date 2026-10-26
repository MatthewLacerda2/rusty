//! Builds the per-frame `PostParams` from one view's camera (#1434): its
//! `CameraComponent` and `VisualCorrectionComponent`, gated by the quality preset.
//!
//! This is the bridge from the editor's "dead knobs" to the GPU: every field the
//! inspector exposes ends up packed here and read by `postfx.wgsl`.
//!
//! **Whose settings.** Every function takes the view's post-FX camera, the base of
//! its camera stack (`render::ScreenStack::base_id`, or a render-texture camera's own
//! entity), the way Unity URP post-processes a camera stack once, with the base
//! camera's settings. A camera without an active volume is neutral; `None` (no scene
//! camera) is too. `Graphics.*` still edits the scene's *first* active camera and
//! volume, the script-facing `Camera.main` shortcut.

use glam::Mat4;

use super::{PostParams, PostPasses};
use crate::components::VisualCorrectionComponent;
use crate::core::settings::quality::QualityPreset;
use crate::scene::Scene;

/// Pack `camera`'s visual-correction volume and camera settings into a `PostParams`
/// for `postfx.wgsl`. Returns the params plus whether the bloom passes should run
/// this frame.
///
/// `prev_view_proj` is last frame's view-projection (camera motion-blur velocity).
pub fn build_post_params(
    scene: &Scene,
    camera: Option<u32>,
    quality: QualityPreset,
    view_proj: Mat4,
    prev_view_proj: Mat4,
    camera_pos: [f32; 3],
) -> (PostParams, bool) {
    let mut params = PostParams {
        view_proj: view_proj.to_cols_array(),
        inv_view_proj: view_proj.inverse().to_cols_array(),
        prev_view_proj: prev_view_proj.to_cols_array(),
        // w: game time (#398), the clock the scene's shaders read as `camera.time`.
        camera_pos: [
            camera_pos[0],
            camera_pos[1],
            camera_pos[2],
            scene.shader_time,
        ],
        ..Default::default()
    };
    let Some((id, vc)) = camera.and_then(|id| Some((id, active_volume(scene, id)?))) else {
        return (params, false);
    };

    params.color = [vc.exposure, vc.contrast, vc.saturation, vc.gamma.max(0.01)];
    params.bloom = [
        vc.bloom_intensity,
        vc.bloom_threshold,
        vc.tonemap.to_index() as f32,
        if vc.bloom_active { 1.0 } else { 0.0 },
    ];

    // SSR mode: 0 off, 2 = real screen-space (High tier only). Low/Medium keep
    // the forward shader's cubemap reflection, so post-FX SSR stays off there.
    let ssr_mode = if vc.ssr_active && quality.screen_space_ssr() {
        2.0
    } else {
        0.0
    };

    // Motion blur is gated by the camera component AND the preset.
    let (mb_active, mb_samples) = match scene.world.camera(id).as_deref() {
        Some(cam) if cam.motion_blur_active && quality.motion_blur() => {
            (1.0, cam.motion_blur_samples.clamp(2, 32) as f32)
        }
        _ => (0.0, 0.0),
    };
    params.flags = [ssr_mode, mb_active, 1.0, 0.0];
    params.misc[3] = mb_samples;
    (params, vc.bloom_active)
}

/// The optional passes this frame runs for `camera`: bloom (decided by
/// [`build_post_params`]), FXAA, and its volume's authored effects.
pub fn post_passes(scene: &Scene, camera: Option<u32>, bloom: bool) -> PostPasses {
    let vc = camera.and_then(|id| active_volume(scene, id));
    PostPasses {
        bloom,
        fxaa: fxaa_enabled(scene, camera),
        custom: vc
            .as_ref()
            .map(|vc| vc.custom_effects.clone())
            .unwrap_or_default(),
        post_params: vc.map(|vc| vc.post_params).unwrap_or_default(),
    }
}

/// `id`'s visual-correction volume, when the entity is active and so is the volume.
pub(crate) fn active_volume(scene: &Scene, id: u32) -> Option<VisualCorrectionComponent> {
    let vc = scene.world.visual_correction(id)?;
    (scene.world.is_active_in_hierarchy(id) && vc.active).then(|| vc.clone())
}

/// Whether the FXAA pass runs this frame for `camera` (#360).
///
/// Deliberately *not* tied to the volume [`build_post_params`] reads: FXAA lives on
/// the camera, so tying it to the volume would make anti-aliasing depend on the
/// camera carrying a `VisualCorrection` — the opposite of a default-on effect.
///
/// No camera component at all gets `true`: default-on means a scene that has never
/// heard of the knob is still anti-aliased.
///
/// Unlike motion blur this is not gated by [`QualityPreset`] — FXAA runs on every
/// tier, Low included, where it is precisely the cheap anti-aliasing an iGPU wants.
pub fn fxaa_enabled(scene: &Scene, camera: Option<u32>) -> bool {
    camera
        .and_then(|id| scene.world.camera(id).map(|c| c.fxaa_active))
        .unwrap_or(true)
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
