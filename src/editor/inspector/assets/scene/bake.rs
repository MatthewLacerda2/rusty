//! The scene inspector's **Generate Lighting** button (#832, Unity 5's button in the
//! Lighting window): lightmaps, then light and reflection probes, with the scene's
//! own lighting settings, and the result written into the scene file. The same
//! `dev::bake::generate_lighting` path as the `Lighting.Generate()` script verb (editor↔API
//! parity, no second path). Dev-only: the bakes write authoring artifacts, so ship
//! builds carry neither.
//!
//! The lightmap step runs in the background (#808): the button starts a
//! [`LightmapBakeJob`], the card shows its progress with a Cancel button, and
//! [`poll`] applies the result on the UI thread when the worker finishes, then runs
//! the probe step and writes the lighting. A cancelled bake writes nothing. The
//! settings under the button (#810's directional mode among them) are the scene's,
//! as `Lighting.SetSettings{…}` edits them.

use crate::editor::widgets::DragRange;
use std::path::Path;
use std::time::Duration;

use egui_phosphor::regular as icon;

use crate::dev::bake::generate_lighting::finish;
use crate::dev::bake::lightmap::{BakeOutcome, LightmapBakeJob};
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::lightmap::BakeSettings;
use crate::scene::{LightingSave, Scene, SceneId};
use crate::scripting::ConsoleLogs;

/// How often the editor repaints while a bake runs, so the bar moves on its own.
const PROGRESS_REPAINT: Duration = Duration::from_millis(100);

/// A background Generate Lighting and the scene it belongs to: the scene's runtime
/// identity and the editor's open scene file when it started. A result for a scene
/// that has since been replaced (File ▸ Open/New, or the follow reloading the file
/// after an outside edit, #1374) or saved elsewhere is dropped: its entity ids no
/// longer mean the same meshes, and its lighting would be written into a file as if
/// it were current.
pub struct ActiveBake {
    job: LightmapBakeJob,
    scene: SceneId,
    path: Option<String>,
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
                job,
                scene: scene.id(),
                path: Some(path),
            });
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

/// Each frame the inspector draws: keep the UI repainting while a bake runs, and once
/// the lightmaps land, apply them, bake the probes and write the lighting. Runs
/// whether or not the scene card is showing, and waits while the scene plays.
pub fn poll(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) {
    let open = editor.current_scene_path();
    let Some(active) = &mut editor.dev.lightmap_bake else {
        return;
    };
    if scene.is_playing() {
        // The World holds the play state; the result waits for Stop's restore (#1374).
        return ctx.request_repaint_after(PROGRESS_REPAINT);
    }
    if active.scene != scene.id() || active.path != open {
        // Its result would land on another scene's entities: drop it (dropping the
        // job stops the worker) rather than apply it.
        editor.dev.lightmap_bake = None;
        log.warn("Generate Lighting dropped: the open scene changed".into());
        return;
    }
    if !active.job.is_finished() {
        return ctx.request_repaint_after(PROGRESS_REPAINT);
    }
    // A bake is no edit (#1631): it neither joins the open Undo step nor ends it.
    scene.unrecorded(|scene| land(editor, scene, log, nav));
}

/// Apply the finished lightmap step, bake the probes and write the lighting.
fn land(editor: &mut EditorUi, scene: &mut Scene, log: &mut ConsoleLogs, nav: &NavigationGraph) {
    let Some(mut active) = editor.dev.lightmap_bake.take() else {
        return;
    };
    let Some(outcome) = active.job.poll(scene) else {
        return;
    };
    let path = active.job.scene_path().to_string();
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

#[cfg(test)]
#[path = "bake_tests.rs"]
mod tests;
