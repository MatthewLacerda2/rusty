//! The scene inspector's **Generate Lighting** button (#832, Unity 5's button in the
//! Lighting window): lightmaps, then light and reflection probes, with the scene's
//! own lighting settings, and the result written into the scene file. The same
//! `dev::bake::generate_lighting` path as the `Lighting.Generate()` script verb (editor↔API
//! parity, no second path). Dev-only: the bakes write authoring artifacts, so ship
//! builds carry neither.
//!
//! The lightmap step runs in the background (#808): the button starts a
//! [`LightmapBakeJob`], the card shows its progress with a Cancel button, and
//! [`poll`] applies the result on the UI thread when the worker finishes. The probe
//! step then runs one step a frame (#1602, Unity's `BakeAsync`): the probes capture
//! through the GPU, so they stay on the UI thread, but no frame waits on more than a
//! reflection probe or a few light probes, and the editor and the MCP keep answering.
//! The bar shows `probe i of n`, the console each pass and probe. Then [`poll`]
//! commits the probes and writes the lighting. A bake cancelled during the lightmaps
//! writes nothing. The probe step bakes from a snapshot of the probes and applies them
//! only at its end, so the scene stays editable while it runs; a probe added, removed
//! or moved meanwhile drops it with nothing of the probes applied. It has no Cancel
//! (#1602 kept it out of scope). The
//! settings under the button (#810's directional mode among them) are the scene's,
//! as `Lighting.SetSettings{…}` edits them.

use crate::editor::widgets::DragRange;
use std::path::Path;

use egui_phosphor::regular as icon;

use crate::dev::bake::lightmap::LightmapBakeJob;
use crate::dev::bake::steps::{LightingBake, Stage};
use crate::editor::EditorUi;
use crate::scene::lighting::lightmap::BakeSettings;
use crate::scene::{Scene, SceneId};
use crate::scripting::ConsoleLogs;

mod poll;
pub use poll::poll;

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

/// Which half of Generate Lighting is running.
enum Step {
    /// The lightmaps, on their worker.
    Lightmaps(LightmapBakeJob),
    /// The probes, a step a frame, once `lightmaps` landed, writing to `path`.
    Probes {
        job: Box<LightingBake>,
        lightmaps: usize,
        path: String,
    },
}

impl ActiveBake {
    /// The scene file the bake writes.
    pub fn scene_path(&self) -> &str {
        match &self.step {
            Step::Lightmaps(job) => job.scene_path(),
            Step::Probes { path, .. } => path,
        }
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    path: &str,
) {
    ui.add_space(8.0);
    let idle = editor.dev.lightmap_bake.is_none();
    let open = is_open_scene(editor, path);
    let button = egui::Button::new(format!("{}  Generate Lighting", icon::LIGHTBULB))
        .min_size(egui::Vec2::new(160.0, 24.0));
    // It writes the scene file, so Play greys it out as it does Save (#1340): the
    // inspector is gated on the *requested* play state, so it can still be drawn on
    // the frame Play starts or stops.
    let generate = ui
        .add_enabled(idle && open && !scene.is_playing(), button)
        .on_hover_text("Bake lightmaps, then light & reflection probes, and save the result")
        .on_disabled_hover_text(if open {
            "Stop Play first, or wait for the bake"
        } else {
            "Open this scene to bake it"
        });
    let changed = ui
        .add_enabled_ui(idle, |ui| {
            draw_settings(ui, &mut scene.lighting_settings.lightmaps)
        })
        .inner;
    editor.is_dirty |= changed;
    if generate.clicked() {
        start(editor, scene, console);
    }
    draw_progress(ui, editor);
}

/// The scene's lightmap knobs; returns whether any changed.
fn draw_settings(ui: &mut egui::Ui, s: &mut BakeSettings) -> bool {
    let mut changed = false;
    let mut row = |ui: &mut egui::Ui, drag: egui::DragValue, label: &str, hover: &str| {
        ui.horizontal(|ui| {
            changed |= ui.add(drag).on_hover_text(hover).changed();
            ui.label(label);
        });
    };
    let texels = egui::DragValue::new(&mut s.texels_per_unit).ranged(BakeSettings::TEXELS_PER_UNIT);
    row(
        ui,
        texels.speed(0.1),
        "Texels per unit",
        "Lightmap resolution",
    );
    let samples = egui::DragValue::new(&mut s.samples).ranged(BakeSettings::SAMPLES);
    row(
        ui,
        samples,
        "Samples",
        "Paths traced per texel; more is smoother and slower",
    );
    let bounces = egui::DragValue::new(&mut s.bounces).ranged(BakeSettings::BOUNCES);
    row(
        ui,
        bounces,
        "Bounces",
        "Surfaces a path bounces off before it stops",
    );
    changed |= ui
        .checkbox(&mut s.directional, "Directional lightmaps")
        .on_hover_text("Bake light directions too, so normal maps show under baked light (#810)")
        .changed();
    changed
}

/// Whether the card's scene file is the editor's open scene. Unity bakes the open
/// scene only, so the button is live on that card alone (#1394). Compared
/// canonically: the same file spelled relative and absolute is one scene, while
/// `DiskText` keys the engine's own writes by exact spelling.
fn is_open_scene(editor: &EditorUi, card: &str) -> bool {
    let Some(open) = editor.current_scene_path() else {
        return false;
    };
    open == card
        || matches!(
            (
                crate::core::paths::canonicalize(Path::new(&open)),
                crate::core::paths::canonicalize(Path::new(card)),
            ),
            (Ok(a), Ok(b)) if a == b
        )
}

/// Start the lightmap step of the live scene in the background. It writes to the
/// open scene's file, never to whichever scene asset the card shows.
fn start(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let Some(path) = editor.current_scene_path() else {
        return console.error("Generate Lighting failed: save the scene first".into());
    };
    let settings = scene.lighting_settings.lightmaps;
    match LightmapBakeJob::start(scene, Some(&path), settings) {
        Ok(job) => {
            console.info("Generating lighting in the background…".into());
            editor.dev.lightmap_bake = Some(ActiveBake {
                step: Step::Lightmaps(job),
                scene: scene.id(),
                path: Some(path),
            });
            scene.lighting_bake_running = true; // a script's bake waits from now on
        }
        Err(e) => console.error(format!("Generate Lighting failed: {e}")),
    }
}

/// The running bake's bar, and its Cancel button during the lightmaps.
fn draw_progress(ui: &mut egui::Ui, editor: &mut EditorUi) {
    let Some(active) = &editor.dev.lightmap_bake else {
        return;
    };
    let job = match &active.step {
        Step::Lightmaps(job) => job,
        Step::Probes { job, .. } => return draw_probe_progress(ui, job.stage()),
    };
    let (done, total) = job.progress();
    let text = match (job.is_cancelled(), total) {
        (true, _) => "Cancelling…".to_string(),
        (false, 0) => "Preparing…".to_string(),
        (false, _) => format!("Lightmaps: {done} / {total} texels"),
    };
    ui.horizontal(|ui| {
        let bar = egui::ProgressBar::new(job.fraction()).text(text);
        ui.add_sized([180.0, 20.0], bar);
        let cancel = egui::Button::new(format!("{}  Cancel", icon::X));
        if ui.add_enabled(!job.is_cancelled(), cancel).clicked() {
            job.cancel();
        }
    });
}

/// The probe step's bar: `probe i of n` of the light probes' pass or the reflections.
fn draw_probe_progress(ui: &mut egui::Ui, stage: Stage) {
    let (done, total, text) = match stage {
        Stage::Light {
            bounce,
            done,
            total,
        } => (
            done,
            total,
            format!("Light probes, bounce {bounce}: {done} / {total}"),
        ),
        Stage::Reflection { done, total } => {
            (done, total, format!("Reflection probes: {done} / {total}"))
        }
        Stage::Done => (1, 1, "Saving…".to_string()),
    };
    let bar = egui::ProgressBar::new(done as f32 / total.max(1) as f32).text(text);
    ui.add_sized([180.0, 20.0], bar);
}

#[cfg(test)]
mod drop_tests;
#[cfg(test)]
mod tests;
