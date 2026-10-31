//! Generate Lighting's per-frame drive (#808, #1602): the lightmaps' worker polled,
//! then the probe step taken one step a frame, then the lighting written.

use std::time::Duration;

use super::Step;
use crate::dev::bake::generate_lighting::{finish, start_probes};
use crate::dev::bake::lightmap::BakeOutcome;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::{LightingSave, Scene};
use crate::scripting::ConsoleLogs;

/// How often the editor repaints while the lightmaps bake, so the bar moves on its
/// own; the probe step repaints at once, a step a frame.
const PROGRESS_REPAINT: Duration = Duration::from_millis(100);

/// Each frame the inspector draws: keep the UI repainting while a bake runs, apply
/// the lightmaps once they land, then take one probe step a frame and write the
/// lighting. Runs whether or not the scene card is showing, and waits while the
/// scene plays.
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
        // The World holds the play state; the bake waits for Stop's restore (#1374).
        return ctx.request_repaint_after(PROGRESS_REPAINT);
    }
    if active.scene != scene.id() || active.path != open {
        // Its result would land on another scene's entities: drop it (dropping the
        // job stops the worker) rather than apply it.
        editor.dev.lightmap_bake = None;
        log.warn("Generate Lighting dropped: the open scene changed".into());
        return;
    }
    match advance(&mut active.step, scene, log, nav) {
        Some(needs_save) => {
            editor.dev.lightmap_bake = None;
            // A file that could not take the lighting alone needs the whole scene saved.
            editor.is_dirty |= needs_save;
        }
        None if matches!(active.step, Step::Probes { .. }) => ctx.request_repaint(),
        None => ctx.request_repaint_after(PROGRESS_REPAINT),
    }
}

/// Move the bake on by a frame. Returns `Some` once it ended (baked, cancelled or
/// failed, each said in the console): whether the scene needs a full save.
fn advance(
    step: &mut Step,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) -> Option<bool> {
    let fail = |log: &mut ConsoleLogs, e: String| {
        log.error(format!("Generate Lighting failed: {e}"));
        Some(false)
    };
    match step {
        Step::Lightmaps(job) => {
            let lightmaps = match job.poll(scene)? {
                BakeOutcome::Baked(n) => n,
                BakeOutcome::Cancelled => {
                    log.info("Generate Lighting cancelled; kept the old lighting".into());
                    return Some(false);
                }
                BakeOutcome::Failed(e) => return fail(log, e),
            };
            let path = job.scene_path().to_string();
            match start_probes(scene, &path, Some(nav)) {
                Ok(job) => {
                    *step = Step::Probes {
                        job: Box::new(job),
                        lightmaps,
                        path,
                    };
                    None
                }
                Err(e) => fail(log, e),
            }
        }
        Step::Probes {
            job,
            lightmaps,
            path,
        } => {
            let mut say = |line| log.info(line);
            match job.step(scene, &mut say) {
                Ok(false) => None,
                Ok(true) => match finish(scene, path, *lightmaps, job.report()) {
                    Ok(report) => {
                        log.info(report.summary());
                        Some(report.saved == LightingSave::NeedsSceneSave)
                    }
                    Err(e) => fail(log, e),
                },
                Err(e) => fail(log, e),
            }
        }
    }
}
