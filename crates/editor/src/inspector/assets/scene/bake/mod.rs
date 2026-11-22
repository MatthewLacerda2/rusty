use std::path::Path;

use egui_phosphor::regular as icon;

use crate::EditorUi;
use rusty_dev::bake::lighting::LightingBakeReport;
use rusty_dev::bake::lightmap::LightmapBakeJob;
use rusty_dev::bake::occlusion::OcclusionBakeJob;
use rusty_dev::bake::steps::LightingBake;
use rusty_navigation::NavigationGraph;
use rusty_scene::{Scene, SceneId};
use rusty_script::scripting::ConsoleLogs;

mod outcome;
mod poll;
mod progress;
mod settings;
mod start;
pub use poll::{poll, poll_undrawn};
use progress::draw_progress;
use settings::{draw_diffuse_model, draw_mixed_mode, draw_settings};
use start::{start, start_occlusion, take_request};

pub struct ActiveBake {
    step: Step,
    scene: SceneId,
    path: Option<String>,
}

enum Step {
    Lightmaps(LightmapBakeJob),
    Probes {
        job: Box<LightingBake>,
        lightmaps: usize,
        path: String,
    },
    Occlusion {
        job: OcclusionBakeJob,
        lightmaps: usize,
        probes: LightingBakeReport,
        saved: rusty_edit::LightingSave,
        path: String,
    },
    /// Bake Occlusion: the PVS alone, on its worker, written to `path`.
    OcclusionOnly { job: OcclusionBakeJob, path: String },
}

impl ActiveBake {
    pub fn scene_path(&self) -> &str {
        match &self.step {
            Step::Lightmaps(job) => job.scene_path(),
            Step::Probes { path, .. }
            | Step::Occlusion { path, .. }
            | Step::OcclusionOnly { path, .. } => path,
        }
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    (path, nav): (&str, &NavigationGraph),
) {
    ui.add_space(8.0);
    let idle = editor.dev.lightmap_bake.is_none();
    let open = is_open_scene(editor, path);
    let button = egui::Button::new(format!("{}  Generate Lighting", icon::LIGHTBULB))
        .min_size(egui::Vec2::new(160.0, 24.0));
    // The inspector follows the requested play state: the scene still plays the frame Stop
    // is pressed.
    let live = idle && open && !scene.is_playing();
    let why = if open {
        "Stop Play first, or wait for the bake"
    } else {
        NOT_OPEN
    };
    let (generate, occlusion) = ui
        .horizontal(|ui| {
            let generate = ui
                .add_enabled(live, button)
                .on_hover_text(
                    "Bake lightmaps, then light & reflection probes, and save the result",
                )
                .on_disabled_hover_text(why);
            let occlusion = egui::Button::new(format!("{}  Bake Occlusion", icon::EYE_SLASH));
            let occlusion = ui
                .add_enabled(live, occlusion)
                .on_hover_text("Bake only the occlusion culling, and save it beside the scene")
                .on_disabled_hover_text(why);
            (generate, occlusion)
        })
        .inner;
    // Greyed out mid-bake: the lightmaps, then the probes, read the diffuse model, and an
    // edit between them would bake two.
    let settings = &mut scene.lighting_settings;
    let changed = ui
        .add_enabled_ui(idle, |ui| {
            draw_settings(ui, &mut settings.lightmaps)
                | draw_diffuse_model(ui, &mut settings.diffuse_model)
                | draw_mixed_mode(ui, &mut settings.mixed_lighting_mode)
        })
        .inner;
    editor.is_dirty |= changed;
    if generate.clicked() {
        start(editor, scene, console);
    }
    if occlusion.clicked() {
        start_occlusion(editor, scene, console, nav);
    }
    draw_progress(ui, editor);
}

pub(super) const NOT_OPEN: &str = "Open this scene to bake it";

/// Whether the card's scene file is the editor's open scene. Unity bakes the open
/// scene only, so the button is live on that card alone. Compared
/// canonically: the same file spelled relative and absolute is one scene, while
/// `DiskText` keys the engine's own writes by exact spelling.
pub(super) fn is_open_scene(editor: &EditorUi, card: &str) -> bool {
    let Some(open) = editor.current_scene_path() else {
        return false;
    };
    open == card
        || matches!(
            (
                std::fs::canonicalize(Path::new(&open)),
                std::fs::canonicalize(Path::new(card)),
            ),
            (Ok(a), Ok(b)) if a == b
        )
}

#[cfg(test)]
pub(crate) mod async_tests;
#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod drop_tests;
#[cfg(test)]
mod occlusion_drop_tests;
#[cfg(test)]
mod occlusion_only_tests;
#[cfg(test)]
mod play_tests;
#[cfg(test)]
mod tests;
