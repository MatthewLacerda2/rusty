//! Generate Lighting's per-frame drive (#808, #1602): the lightmaps' worker polled
//! and its lightmaps applied, then the probe step taken one step a frame and
//! committed and the lighting written, then the occlusion culling's worker polled
//! (#1714) and its PVS written beside it. A bake dropped midway says what reached
//! the scene: the lightmaps once they landed, the probes and the written lighting
//! once committed, the PVS never. A failure before the lighting is written leaves
//! it unwritten, so it marks the scene unsaved.

use std::time::Duration;

use super::outcome::{done, dropped, fail, fail_occlusion, verb};
use super::Step;
use crate::dev::bake::generate_lighting::{apply_occlusion, finish, start_probes, write_lighting};
use crate::dev::bake::lightmap::BakeOutcome;
use crate::dev::bake::occlusion::OcclusionBakeJob;
use crate::dev::bake::steps::PROBES_CHANGED;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::bake_status::BakeStatus;
use crate::scene::{LightingSave, Scene};
use crate::scripting::ConsoleLogs;

/// How often the editor repaints while the lightmaps bake, so the bar moves on its
/// own; the probe step repaints at once, a step a frame.
const PROGRESS_REPAINT: Duration = Duration::from_millis(100);

/// Each frame the inspector draws: keep the UI repainting while a bake runs, apply
/// the lightmaps once they land, then take one probe step a frame and write the
/// lighting. Starts the button's job when a script asked (`Lighting.GenerateAsync`,
/// #1999). Runs whether or not the scene card is showing, and waits while the
/// scene plays. Tells the scene whether a bake is running, so a script's bake waits
/// its turn.
pub fn poll(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) {
    super::take_request(editor, scene, log);
    drive(editor, ctx, scene, log, nav);
    scene.lighting_bake_running = editor.dev.lightmap_bake.is_some();
}

fn drive(
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
        // Its result would land on another scene's entities, or be written into a
        // file the scene no longer is: drop it (dropping the job stops the worker and
        // deletes its staged cubemaps) rather than apply it.
        // A replaced scene took any applied lightmaps with it; only Save As keeps
        // them on the scene in front of the user.
        let line = match active.scene != scene.id() {
            true => format!("{} dropped: the open scene changed", verb(&active.step)),
            false => dropped(&active.step, "the scene was saved elsewhere"),
        };
        log.warn(line.clone());
        scene.lighting_bake_status = BakeStatus::Failed(line);
        if let (Step::Lightmaps(job), true) = (&mut active.step, active.scene == scene.id()) {
            // Still the scene in front of the user: put its lightmaps back (#1970).
            scene.unrecorded(|scene| job.restore(scene));
        }
        editor.dev.lightmap_bake = None;
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
    let job = match step {
        Step::Lightmaps(job) => job,
        Step::Probes { .. } => return advance_probes(step, scene, log),
        Step::Occlusion { .. } => return advance_occlusion(step, scene, log),
        Step::OcclusionOnly { .. } => return advance_occlusion_only(step, scene, log),
    };
    if !job.is_finished() {
        // A progressive bake's newest pass on the live scene (#1970): no edit.
        scene.unrecorded(|scene| job.show_preview(scene));
        return None;
    }
    // A bake is no edit (#1631): what it writes never joins an Undo step.
    scene.unrecorded(|scene| land_lightmaps(step, scene, log, nav))
}

/// Apply the finished lightmaps, then start the probe step (planning the probe
/// sets it applies when it commits).
fn land_lightmaps(
    step: &mut Step,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) -> Option<bool> {
    let Step::Lightmaps(job) = step else {
        return None;
    };
    let lightmaps = match job.poll(scene)? {
        BakeOutcome::Baked(n) => n,
        BakeOutcome::Cancelled => {
            let line = "Generate Lighting cancelled; kept the old lighting".to_string();
            log.info(line.clone());
            scene.lighting_bake_status = BakeStatus::Cancelled(line);
            return Some(false);
        }
        BakeOutcome::Failed(e) => return fail(scene, log, e, false),
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
        Err(e) => fail(scene, log, e, true), // the lightmaps were applied but never written
    }
}

/// One probe step; at the end, commit the probes, write the lighting and start the
/// occlusion bake.
fn advance_probes(step: &mut Step, scene: &mut Scene, log: &mut ConsoleLogs) -> Option<bool> {
    let Step::Probes {
        job,
        lightmaps,
        path,
    } = step
    else {
        return None;
    };
    let mut say = |line| log.info(line);
    let stepped = job.step(scene, &mut say).and_then(|done| match done {
        true => scene.unrecorded(|scene| job.commit(scene)).map(|()| true),
        false => Ok(false),
    });
    match stepped {
        Ok(false) => None,
        Ok(true) => {
            let saved = match write_lighting(scene, path) {
                Ok(saved) => saved,
                Err(e) => return fail(scene, log, e, true),
            };
            let needs_save = saved == LightingSave::NeedsSceneSave;
            match OcclusionBakeJob::start(scene) {
                Ok(occlusion) => {
                    *step = Step::Occlusion {
                        job: occlusion,
                        lightmaps: *lightmaps,
                        probes: job.report(),
                        saved,
                        path: std::mem::take(path),
                    };
                    None
                }
                Err(e) => fail(scene, log, e, needs_save),
            }
        }
        Err(e) if e == PROBES_CHANGED => {
            let line = dropped(step, "the probes changed during the bake");
            log.warn(line.clone());
            scene.lighting_bake_status = BakeStatus::Failed(line);
            // The lightmaps were applied but never written.
            Some(true)
        }
        Err(e) => fail(scene, log, e, true),
    }
}

/// Once the occlusion worker finished, apply its PVS and write it beside the
/// scene; the lighting was written when the probes committed.
fn advance_occlusion(step: &mut Step, scene: &mut Scene, log: &mut ConsoleLogs) -> Option<bool> {
    let Step::Occlusion {
        job,
        lightmaps,
        probes,
        saved,
        path,
    } = step
    else {
        return None;
    };
    let needs_save = *saved == LightingSave::NeedsSceneSave;
    let baked = match job.poll()? {
        Ok(baked) => baked,
        Err(e) => return fail(scene, log, e, needs_save),
    };
    let lighting = (*lightmaps, *probes, *saved);
    match scene.unrecorded(|scene| finish(scene, path, lighting, baked)) {
        Ok(report) => Some(done(scene, log, report.summary(), needs_save)),
        Err(e) => fail(scene, log, e, true), // the PVS applied, never written
    }
}

/// Bake Occlusion (#1958): once its worker finished, apply the PVS and write it
/// beside the scene. A failed bake applied nothing; a PVS applied but never
/// written needs the scene saved, which writes it (as `advance_occlusion` does).
fn advance_occlusion_only(
    step: &mut Step,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
) -> Option<bool> {
    let Step::OcclusionOnly { job, path } = step else {
        return None;
    };
    let baked = match job.poll()? {
        Ok(baked) => baked,
        Err(e) => return fail_occlusion(scene, log, e, false),
    };
    match scene.unrecorded(|scene| apply_occlusion(scene, path, baked)) {
        Ok(report) => Some(done(scene, log, report.summary(), false)),
        // `drive` never advances during Play, so only the write can fail here, with
        // the PVS already on the scene.
        Err(e) => fail_occlusion(scene, log, e, true),
    }
}
