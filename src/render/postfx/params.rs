//! Builds the per-frame `PostParams` from one view's camera (#1434): its
//! `CameraComponent` and `PostProcessingComponent`, gated by the quality preset.
//!
//! This is the bridge from the editor's "dead knobs" to the GPU: every field the
//! inspector exposes ends up packed here and read by `postfx.wgsl`.
//!
//! **Whose settings.** A view post-processes with its base camera, the way Unity URP
//! post-processes a camera stack once, with the base camera's settings: the screen
//! stack's `scene::camera::main_camera` (also the camera `Graphics.*` edits), or a
//! render-texture camera's own entity. The stack borrows that camera's components
//! once, as a [`PostSource`], and every function here reads them through it. A
//! camera without an active volume is neutral; a view with no camera is too.

use std::collections::BTreeMap;

use glam::Mat4;

use super::{PostParams, PostPasses};
use crate::components::{CameraComponent, PostProcessingComponent};
use crate::core::settings::quality::QualityPreset;
use crate::scene::camera::camera_volume;
use crate::scene::Scene;

/// The components one view post-processes with, borrowed once per view (#1434).
#[derive(Clone, Copy, Default)]
pub(crate) struct PostSource<'a> {
    /// The base camera's component: motion blur and FXAA.
    pub camera: Option<&'a CameraComponent>,
    /// Its post-processing volume, when active: grade, bloom, SSR, SSAO, shadows
    /// and the authored effects. `None` runs them neutral.
    pub volume: Option<&'a PostProcessingComponent>,
}

/// The base camera `id`'s components, borrowed for a view: the guards the caller
/// keeps alive while it draws, and [`PostSource`] reads through them.
pub(crate) fn borrow_post_source(scene: &Scene, id: Option<u32>) -> PostGuards<'_> {
    PostGuards {
        camera: id.and_then(|id| scene.world.camera(id)),
        volume: id.and_then(|id| camera_volume(scene, id)),
    }
}

/// The component guards behind a [`PostSource`].
pub(crate) struct PostGuards<'w> {
    camera: Option<crate::ecs::CompRef<'w, CameraComponent>>,
    volume: Option<crate::ecs::CompRef<'w, PostProcessingComponent>>,
}

impl PostGuards<'_> {
    pub(crate) fn source(&self) -> PostSource<'_> {
        PostSource {
            camera: self.camera.as_deref(),
            volume: self.volume.as_deref(),
        }
    }
}

/// Pack `post`'s post-processing volume and camera settings into a `PostParams`
/// for `postfx.wgsl`. Returns the params plus whether the bloom passes should run
/// this frame.
///
/// `prev_view_proj` is last frame's view-projection (camera motion-blur velocity).
pub(crate) fn build_post_params(
    scene: &Scene,
    post: PostSource,
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
    let Some(vc) = post.volume else {
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
    let samples =
        CameraComponent::MIN_MOTION_BLUR_SAMPLES..=CameraComponent::MAX_MOTION_BLUR_SAMPLES;
    let (mb_active, mb_samples) = match post.camera {
        Some(cam) if cam.motion_blur_active && quality.motion_blur() => (
            1.0,
            cam.motion_blur_samples
                .clamp(*samples.start(), *samples.end()) as f32,
        ),
        _ => (0.0, 0.0),
    };
    params.flags = [ssr_mode, mb_active, 1.0, 0.0];
    params.misc[3] = mb_samples;
    (params, vc.bloom_active)
}

/// No authored-effect params: a view without a volume.
static NO_POST_PARAMS: BTreeMap<String, Vec<f32>> = BTreeMap::new();

/// The optional passes this frame runs for `post`: bloom (decided by
/// [`build_post_params`]), FXAA, and its volume's authored effects, borrowed.
pub(crate) fn post_passes<'a>(post: PostSource<'a>, bloom: bool) -> PostPasses<'a> {
    PostPasses {
        bloom,
        fxaa: fxaa_enabled(post),
        custom: post.volume.map_or(&[], |vc| &vc.custom_effects),
        post_params: post.volume.map_or(&NO_POST_PARAMS, |vc| &vc.post_params),
    }
}

/// Whether the FXAA pass runs this frame for `post`'s camera (#360).
///
/// Deliberately *not* tied to the volume [`build_post_params`] reads: FXAA lives on
/// the camera, so tying it to the volume would make anti-aliasing depend on the
/// camera carrying a `PostProcessing` — the opposite of a default-on effect.
///
/// No camera component at all gets `true`: default-on means a scene that has never
/// heard of the knob is still anti-aliased.
///
/// Unlike motion blur this is not gated by [`QualityPreset`] — FXAA runs on every
/// tier, Low included, where it is precisely the cheap anti-aliasing an iGPU wants.
pub(crate) fn fxaa_enabled(post: PostSource) -> bool {
    post.camera.is_none_or(|c| c.fxaa_active)
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
