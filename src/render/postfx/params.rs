//! Whose settings a view post-processes with (#1434), and the one gate every
//! effect asks ([`effect_runs`], #1438): its camera's choice, the player's option and
//! the quality tier. Each effect packs its own params from them
//! (`postfx::effects`, #1467); [`frame_params`] is what every post pass shares.
//!
//! **Whose settings.** A view post-processes with its base camera, the way Unity URP
//! post-processes a camera stack once, with the base camera's settings: the screen
//! stack's `scene::camera::main_camera` (also the camera `Graphics.*` edits), or a
//! render-texture camera's own entity. The stack borrows that camera's components
//! once, as a [`PostSource`], and every function here reads them through it. A
//! camera without an active volume is neutral; a view with no camera is too.

use glam::Mat4;

use super::PostParams;
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

/// The frame every post pass shares: the view-projection both ways, the camera's
/// position and the game time (#398), the clock the scene's shaders read as
/// `camera.time`.
pub(crate) fn frame_params(scene: &Scene, view_proj: Mat4, camera_pos: [f32; 3]) -> PostParams {
    PostParams {
        view_proj: view_proj.to_cols_array(),
        inv_view_proj: view_proj.inverse().to_cols_array(),
        camera_pos: [
            camera_pos[0],
            camera_pos[1],
            camera_pos[2],
            scene.shader_time,
        ],
    }
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
