//! The scene inspector's **Generate Lighting** button (#832, Unity 5's button in the
//! Lighting window): lightmaps, then light and reflection probes, with the scene's
//! own lighting settings, and the result written into the scene file. The same
//! `dev::generate_lighting` path as the `Lighting.Generate()` script verb (editor↔API
//! parity, no second path). Dev-only: the bakes write authoring artifacts, so ship
//! builds carry neither.
//!
//! The lightmap step runs in the background (#808): the button starts a
//! [`LightmapBakeJob`], the card shows its progress with a Cancel button, and
//! [`poll`] applies the result on the UI thread when the worker finishes, then runs
//! the probe step and writes the lighting. A cancelled bake writes nothing. The
//! settings under the button (#810's directional mode among them) are the scene's,
//! as `Lighting.SetSettings{…}` edits them.

use std::time::Duration;

use egui_phosphor::regular as icon;

use crate::dev::generate_lighting::finish;
use crate::dev::lightmap_bake::{BakeOutcome, LightmapBakeJob};
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::lightmap::BakeSettings;
use crate::scene::{LightingSave, Scene};
use crate::scripting::ConsoleLogs;

/// How often the editor repaints while a bake runs, so the bar moves on its own.
const PROGRESS_REPAINT: Duration = Duration::from_millis(100);

/// A background Generate Lighting and the scene it belongs to: the editor's open
/// scene file when it started. A result for a scene that has since been swapped out
/// is dropped, because its entity ids no longer mean the same meshes.
pub struct ActiveBake {
    job: LightmapBakeJob,
    scene: Option<String>,
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
    let button = egui::Button::new(format!("{}  Generate Lighting", icon::LIGHTBULB))
        .min_size(egui::Vec2::new(160.0, 24.0));
    let generate = ui
        .add_enabled(idle, button)
        .on_hover_text("Bake lightmaps, then light & reflection probes, and save the result");
    let changed = ui
        .add_enabled_ui(idle, |ui| {
            draw_settings(ui, &mut scene.lighting_settings.lightmaps)
        })
        .inner;
    editor.is_dirty |= changed;
    if generate.clicked() {
        start(editor, scene, console, path);
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
    let texels = egui::DragValue::new(&mut s.texels_per_unit).range(0.25..=64.0);
    row(
        ui,
        texels.speed(0.1),
        "Texels per unit",
        "Lightmap resolution",
    );
    let samples = egui::DragValue::new(&mut s.samples).range(1..=4096);
    row(
        ui,
        samples,
        "Samples",
        "Paths traced per texel; more is smoother and slower",
    );
    let bounces = egui::DragValue::new(&mut s.bounces).range(1..=16);
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

/// Start the lightmap step of the live scene in the background.
fn start(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs, path: &str) {
    let settings = scene.lighting_settings.lightmaps;
    match LightmapBakeJob::start(scene, Some(path), settings) {
        Ok(job) => {
            console.info("Generating lighting in the background…".into());
            let scene = editor.current_scene_path.clone();
            editor.dev.lightmap_bake = Some(ActiveBake { job, scene });
        }
        Err(e) => console.error(format!("Generate Lighting failed: {e}")),
    }
}

/// The running bake's bar and its Cancel button, under the button.
fn draw_progress(ui: &mut egui::Ui, editor: &mut EditorUi) {
    let Some(active) = &editor.dev.lightmap_bake else {
        return;
    };
    let job = &active.job;
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

/// Each edit-mode frame: keep the UI repainting while a bake runs, and once the
/// lightmaps land, apply them, bake the probes and write the lighting. Runs whether
/// or not the scene card is showing.
pub fn poll(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) {
    let Some(active) = &mut editor.dev.lightmap_bake else {
        return;
    };
    if active.scene != editor.current_scene_path {
        // Its result would land on another scene's entities: drop it (dropping the
        // job stops the worker) rather than apply it.
        editor.dev.lightmap_bake = None;
        log.warn("Generate Lighting dropped: the open scene changed".into());
        return;
    }
    let Some(outcome) = active.job.poll(scene) else {
        ctx.request_repaint_after(PROGRESS_REPAINT);
        return;
    };
    let path = active.job.scene_path().to_string();
    editor.dev.lightmap_bake = None;
    let lightmaps = match outcome {
        BakeOutcome::Baked(n) => n,
        BakeOutcome::Cancelled => {
            return log.info("Generate Lighting cancelled; kept the old lighting".into())
        }
        BakeOutcome::Failed(e) => return log.error(format!("Generate Lighting failed: {e}")),
    };
    match finish(scene, &path, Some(nav), lightmaps) {
        Ok(report) => {
            // A file that could not take the lighting alone needs the whole scene saved.
            editor.is_dirty |= report.saved == LightingSave::NeedsSceneSave;
            log.info(report.summary());
        }
        Err(e) => log.error(format!("Generate Lighting failed: {e}")),
    }
}
