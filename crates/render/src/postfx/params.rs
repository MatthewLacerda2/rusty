use glam::Mat4;

use super::PostParams;
use rusty_components::{CameraComponent, PostProcessingComponent};
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::GraphicsSettings;
use rusty_scene::camera::camera_volume;
use rusty_scene::Scene;

#[derive(Clone, Copy, Default)]
pub(crate) struct PostSource<'a> {
    pub camera: Option<&'a CameraComponent>,
    pub volume: Option<&'a PostProcessingComponent>,
}

pub(crate) fn borrow_post_source(scene: &Scene, id: Option<u32>) -> PostGuards<'_> {
    PostGuards {
        camera: id.and_then(|id| scene.world.camera(id)),
        volume: id.and_then(|id| camera_volume(scene, id)),
    }
}

pub(crate) struct PostGuards<'w> {
    camera: Option<rusty_ecs::CompRef<'w, CameraComponent>>,
    volume: Option<rusty_ecs::CompRef<'w, PostProcessingComponent>>,
}

impl PostGuards<'_> {
    pub(crate) fn source(&self) -> PostSource<'_> {
        PostSource {
            camera: self.camera.as_deref(),
            volume: self.volume.as_deref(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Gate {
    pub option: Option<GraphicsOption>,
}

impl Gate {
    pub(crate) const NONE: Gate = Gate { option: None };

    pub(crate) const fn option(option: GraphicsOption) -> Gate {
        Gate {
            option: Some(option),
        }
    }
}

/// The one place that decides it: every gated pass asks here.
pub(crate) fn effect_runs(graphics: GraphicsSettings, gate: Gate, on: bool) -> bool {
    on && gate.option.is_none_or(|o| graphics.options.allows(o))
}

/// `time` is the frame's game clock, `CaptureMode::shader_time`.
pub(crate) fn frame_params(time: f32, view_proj: Mat4, camera_pos: [f32; 3]) -> PostParams {
    PostParams {
        view_proj: view_proj.to_cols_array(),
        inv_view_proj: view_proj.inverse().to_cols_array(),
        camera_pos: [camera_pos[0], camera_pos[1], camera_pos[2], time],
    }
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
