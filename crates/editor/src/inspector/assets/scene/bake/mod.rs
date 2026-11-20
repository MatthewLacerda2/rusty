//! The scene inspector's **Generate Lighting** button (#832, Unity 5's button in the
//! Lighting window): lightmaps, then light and reflection probes, with the scene's
//! own lighting settings, and the result written into the scene file. The same
//! `dev::bake::generate_lighting` path as the `Lighting.Generate()` script verb (editor↔API
//! parity, no second path). Dev-only: the bakes write authoring artifacts, so ship
//! builds carry neither.
//!
//! The lightmap step runs in the background (#808): the button starts a
//! [`LightmapBakeJob`], the card shows its progress with a Cancel button, and
//! [`poll`] applies the result on the UI thread when the worker finishes. The bake
//! is progressive (#1970): [`poll`] shows each finished pass on the live scene, the
//! card says the pass, the samples so far and the time left, and the Scene view's
//! camera is the bake's Prioritize View. The probe
//! step then runs one step a frame (#1602, Unity's `BakeAsync`): the probes capture
//! through the GPU, so they stay on the UI thread, but no frame waits on more than a
//! reflection probe or a few light probes, and the editor and the MCP keep answering.
//! Last, the occlusion culling's PVS bakes on a worker (#1714), its cells on the bar.
//! The card shows the stage (the light probes' bounce, or the reflection probes) on
//! a line above the bar and `i / n` in it; the console says each pass and probe. Then [`poll`]
//! commits the probes and writes the lighting, before the PVS bakes. A bake cancelled during the lightmaps
//! writes nothing. The probe step bakes from a snapshot of the probes and applies them
//! only at its end, so the scene stays editable while it runs; a probe added, removed
//! or moved meanwhile drops it with nothing of the probes applied. It has no Cancel
//! (#1602 kept it out of scope). The
//! settings under the button (#810's directional mode among them) are the scene's,
//! as `Lighting.SetSettings{…}` edits them.
//!
//! A script's `Lighting.GenerateAsync()` (#1999, Unity's `Lightmapping.BakeAsync`)
//! presses the button: it sets `Scene::lighting_bake_requested`, and [`poll`] starts
//! the same job on the next frame. A frame the window cannot draw (a locked screen)
//! still polls ([`poll_undrawn`], through `EditorUi::frame_work`), so the job starts
//! and ends with nobody at the screen.
//!
//! **Bake Occlusion** beside it (#1958, Unity's Occlusion window) runs only the
//! last step, the PVS on its worker, and writes it beside the scene: the same
//! `generate_lighting::apply_occlusion` as `Lighting.BakeOcclusion()`.

use std::path::Path;

use egui_phosphor::regular as icon;

use crate::EditorUi;
use rusty_dev::bake::lighting::LightingBakeReport;
use rusty_dev::bake::lightmap::LightmapBakeJob;
use rusty_dev::bake::occlusion::unsaved_materials_warning;
use rusty_dev::bake::occlusion::{reach, OcclusionBakeJob};
use rusty_dev::bake::steps::LightingBake;
use rusty_lighting::lightmap::BakeView;
use rusty_navigation::NavigationGraph;
use rusty_scene::lighting::bake_status::{BakeProgress, BakeStatus};
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

/// A background Generate Lighting and the scene it belongs to: the scene's runtime
/// identity and the editor's open scene file when it started. A result for a scene
/// that has since been replaced (File ▸ Open/New, or the follow reloading the file
/// after an outside edit, #1374) or saved elsewhere is dropped: its entity ids no
/// longer mean the same meshes, and its lighting would be written into a file as if
/// it were current.
pub struct ActiveBake {
    step: Step,
    scene: SceneId,
    path: Option<String>,
}

/// Which part of Generate Lighting is running.
enum Step {
    /// The lightmaps, on their worker.
    Lightmaps(LightmapBakeJob),
    /// The probes, a step a frame, once `lightmaps` landed, writing to `path`.
    Probes {
        job: Box<LightingBake>,
        lightmaps: usize,
        path: String,
    },
    /// The occlusion culling's PVS, on its worker, once the probes committed and
    /// the lighting was written (`saved` says whether the file took it).
    Occlusion {
        job: OcclusionBakeJob,
        lightmaps: usize,
        probes: LightingBakeReport,
        saved: rusty_edit::LightingSave,
        path: String,
    },
    /// Bake Occlusion (#1958): the PVS alone, on its worker, written to `path`.
    OcclusionOnly { job: OcclusionBakeJob, path: String },
}

impl ActiveBake {
    /// The scene file the bake writes.
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
    // It writes the scene file, so Play greys it out as it does Save (#1340): the
    // inspector is gated on the *requested* play state, so it can still be drawn on
    // the frame Play starts or stops.
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
    // The diffuse model is greyed out with the knobs: lightmaps and probes bake one
    // after the other, and a change between them would save two models as one.
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

/// Why a bake button is greyed out on a scene card that is not the open scene.
pub(super) const NOT_OPEN: &str = "Open this scene to bake it";

/// Whether the card's scene file is the editor's open scene. Unity bakes the open
/// scene only, so the button is live on that card alone (#1394). Compared
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
