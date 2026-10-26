//! Builds the per-frame `PostParams` from one view's camera (#1434): its
//! `CameraComponent` and `PostProcessingComponent`, gated by the quality preset and
//! the player's graphics options through [`effect_runs`] (#1438).
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

use super::motion_blur::BlurParams;
use super::{PostParams, PostPasses};
use crate::components::{CameraComponent, PostProcessingComponent};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::GraphicsSettings;
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

/// Does `effect` run for a view whose camera has it `on` (#1438)? Only when the
/// camera has it on **and** the player's option allows it **and** the quality tier
/// runs it. The one place that decides it: every gated pass asks here.
pub(crate) fn effect_runs(graphics: GraphicsSettings, effect: GraphicsOption, on: bool) -> bool {
    on && graphics.options.allows(effect) && graphics.quality.allows(effect)
}

/// Pack `post`'s post-processing volume into a `PostParams` for `postfx.wgsl`.
/// Returns the params plus whether the bloom passes should run this frame. Motion
/// blur, the camera's, is [`blur_params`].
pub(crate) fn build_post_params(
    scene: &Scene,
    post: PostSource,
    graphics: GraphicsSettings,
    view_proj: Mat4,
    camera_pos: [f32; 3],
) -> (PostParams, bool) {
    let mut params = PostParams {
        view_proj: view_proj.to_cols_array(),
        inv_view_proj: view_proj.inverse().to_cols_array(),
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

    let bloom = effect_runs(graphics, GraphicsOption::Bloom, vc.bloom_active);
    params.color = [vc.exposure, vc.contrast, vc.saturation, vc.gamma.max(0.01)];
    params.bloom = [
        vc.bloom_intensity,
        vc.bloom_threshold,
        vc.tonemap.to_index() as f32,
        if bloom { 1.0 } else { 0.0 },
    ];

    // SSR mode: 0 off, 2 = real screen-space (High tier only). Low/Medium keep
    // the forward shader's cubemap reflection, so post-FX SSR stays off there.
    let ssr_mode = if effect_runs(graphics, GraphicsOption::Ssr, vc.ssr_active) {
        2.0
    } else {
        0.0
    };
    params.flags[0] = ssr_mode;
    (params, bloom)
}

/// The motion blur `post`'s camera runs this frame (#1436), for a view `height`
/// pixels tall; `None` when [`motion_blur_enabled`] does not hold. Like FXAA it
/// lives on the camera, so it needs no volume.
pub(crate) fn blur_params(
    post: PostSource,
    graphics: GraphicsSettings,
    height: u32,
) -> Option<BlurParams> {
    let cam = post
        .camera
        .filter(|_| motion_blur_enabled(post, graphics))?;
    Some(BlurParams::new(cam, height))
}

/// Whether `post`'s camera runs motion blur, through [`effect_runs`]: its component
/// has it on, the player's option allows it and the tier runs it. The velocity
/// target is written exactly when this holds (#1435), so a player who turned motion
/// blur off pays for no velocity pass.
pub(crate) fn motion_blur_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    post.camera.is_some_and(|cam| {
        effect_runs(graphics, GraphicsOption::MotionBlur, cam.motion_blur_active)
    })
}

/// No authored-effect params: a view without a volume.
static NO_POST_PARAMS: BTreeMap<String, Vec<f32>> = BTreeMap::new();

/// The optional passes this frame runs for `post`: bloom (decided by
/// [`build_post_params`]), FXAA, and its volume's authored effects, borrowed.
pub(crate) fn post_passes<'a>(
    post: PostSource<'a>,
    bloom: bool,
    graphics: GraphicsSettings,
) -> PostPasses<'a> {
    PostPasses {
        bloom,
        fxaa: fxaa_enabled(post, graphics),
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
/// Unlike motion blur no quality tier turns it off — FXAA runs on every tier, Low
/// included, where it is precisely the cheap anti-aliasing an iGPU wants; the
/// player's `fxaa` option can.
pub(crate) fn fxaa_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    let on = post.camera.is_none_or(|c| c.fxaa_active);
    effect_runs(graphics, GraphicsOption::Fxaa, on)
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
