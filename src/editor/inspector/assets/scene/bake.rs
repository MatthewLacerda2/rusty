//! The scene inspector's bake buttons: "Bake Lighting" (#246, probes) and "Bake
//! Lightmaps" (#438). Both route through the same `dev` paths as the `Lighting.Bake`
//! / `Lighting.BakeLightmaps` script verbs (editor↔API parity, no second path).
//! Dev-only: the bakes write authoring artifacts, so ship builds carry neither.
//!
//! The lightmap bake runs in the background (#808), as Unity's does: the button
//! starts a [`LightmapBakeJob`], the card shows its progress with a Cancel button,
//! and [`poll`] applies the result on the UI thread when the worker finishes. The
//! script verb stays synchronous.

use std::time::Duration;

use egui_phosphor::regular as icon;

use crate::dev::lightmap_bake::{BakeOutcome, LightmapBakeJob};
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::lightmap::BakeSettings;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// How often the editor repaints while a bake runs, so the bar moves on its own.
const PROGRESS_REPAINT: Duration = Duration::from_millis(100);

/// A background lightmap bake and the scene it belongs to: the editor's open scene
/// file when it started. A result for a scene that has since been swapped out is
/// dropped, because its entity ids no longer mean the same meshes.
pub struct ActiveBake {
    job: LightmapBakeJob,
    scene: Option<String>,
}

pub fn draw(
    ui: &mut egui::Ui,
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &NavigationGraph,
    path: &str,
) {
    ui.add_space(8.0);
    let size = egui::Vec2::new(120.0, 24.0);
    let (probes, lightmaps) = ui
        .horizontal(|ui| {
            let probes = egui::Button::new(format!("{}  Bake Lighting", icon::LIGHTBULB));
            let probes = ui.add(probes.min_size(size));
            let probes = probes.on_hover_text("Auto-place + bake light & reflection probes");
            let idle = editor.dev.lightmap_bake.is_none();
            let maps = egui::Button::new(format!("{}  Bake Lightmaps", icon::MAP_TRIFOLD));
            let maps = ui.add_enabled(idle, maps.min_size(size));
            let maps = maps.on_hover_text("Bake static meshes with a second UV map (#438)");
            (probes.clicked(), maps.clicked())
        })
        .inner;
    if lightmaps {
        start(editor, scene, console, path);
    }
    draw_progress(ui, editor);
    if !probes {
        return;
    }
    let params = crate::dev::lighting_bake::LightingBakeParams::default();
    match crate::dev::lighting_bake::bake_lighting(scene, Some(path), Some(nav), params) {
        Ok(r) => console.info(format!(
            "Baked lighting: {} light probe(s), {} reflection probe(s)",
            r.light_probes, r.reflection_probes
        )),
        Err(e) => console.error(format!("Bake Lighting failed: {}", e)),
    }
}

/// Start a background bake of the live scene, its pages written beside `path`.
fn start(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs, path: &str) {
    match LightmapBakeJob::start(scene, Some(path), BakeSettings::default()) {
        Ok(job) => {
            console.info("Baking lightmaps in the background…".into());
            let scene = editor.current_scene_path.clone();
            editor.dev.lightmap_bake = Some(ActiveBake { job, scene });
        }
        Err(e) => console.error(format!("Bake Lightmaps failed: {e}")),
    }
}

/// The running bake's bar and its Cancel button, under the bake buttons.
fn draw_progress(ui: &mut egui::Ui, editor: &mut EditorUi) {
    let Some(active) = &editor.dev.lightmap_bake else {
        return;
    };
    let job = &active.job;
    let (done, total) = job.progress();
    let text = match (job.is_cancelled(), total) {
        (true, _) => "Cancelling…".to_string(),
        (false, 0) => "Preparing…".to_string(),
        (false, _) => format!("{done} / {total} texels"),
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

/// Each edit-mode frame: keep the UI repainting while a bake runs, and apply its
/// result once the worker is done. Runs whether or not the scene card is showing.
pub fn poll(editor: &mut EditorUi, ctx: &egui::Context, scene: &mut Scene, log: &mut ConsoleLogs) {
    let Some(active) = &mut editor.dev.lightmap_bake else {
        return;
    };
    if active.scene != editor.current_scene_path {
        // Its result would land on another scene's entities: drop it (dropping the
        // job stops the worker) rather than apply it.
        editor.dev.lightmap_bake = None;
        log.warn("Lightmap bake dropped: the open scene changed".into());
        return;
    }
    let Some(outcome) = active.job.poll(scene) else {
        ctx.request_repaint_after(PROGRESS_REPAINT);
        return;
    };
    editor.dev.lightmap_bake = None;
    match outcome {
        BakeOutcome::Baked(n) => log.info(format!("Baked {n} lightmap(s); save the scene")),
        BakeOutcome::Cancelled => {
            log.info("Lightmap bake cancelled; kept the old lightmaps".into())
        }
        BakeOutcome::Failed(e) => log.error(format!("Bake Lightmaps failed: {e}")),
    }
}
