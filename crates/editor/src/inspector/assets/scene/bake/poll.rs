use std::time::{Duration, Instant};

use super::outcome::{done, dropped, fail, fail_occlusion, verb};
use super::Step;
use crate::EditorUi;
use rusty_dev::bake::generate_lighting::{apply_occlusion, finish, start_probes, write_lighting};
use rusty_dev::bake::lightmap::BakeOutcome;
use rusty_dev::bake::occlusion::{reach, OcclusionBakeJob};
use rusty_dev::bake::steps::PROBES_CHANGED;
use rusty_edit::LightingSave;
use rusty_navigation::NavigationGraph;
use rusty_scene::lighting::bake_status::BakeStatus;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

const PROGRESS_REPAINT: Duration = Duration::from_millis(100);

/// How long a frame the window cannot draw spends on probe steps ([`poll_undrawn`]):
/// under the pacer's 100 ms retry (`shell::pace`).
pub const UNDRAWN_PROBE_BUDGET: Duration = Duration::from_millis(80);

pub fn poll(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) {
    paced(editor, ctx, scene, log, nav, None);
}

pub fn poll_undrawn(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) {
    paced(editor, ctx, scene, log, nav, Some(UNDRAWN_PROBE_BUDGET));
}

/// [`poll`], taking probe steps until `budget` ran out (`None`: one step).
pub(super) fn paced(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
    budget: Option<Duration>,
) {
    let was_busy = scene.bake_busy();
    super::take_request(editor, scene, log);
    let until = budget.map(|b| Instant::now() + b);
    drive(editor, ctx, scene, log, nav, until);
    scene.lighting_bake_running = editor.dev.lightmap_bake.is_some();
    if let Some(active) = &editor.dev.lightmap_bake {
        scene.lighting_bake_status = BakeStatus::Running(active.progress());
    }
    if was_busy && !scene.bake_busy() {
        ctx.request_repaint();
    }
}

fn drive(
    editor: &mut EditorUi,
    ctx: &egui::Context,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
    until: Option<Instant>,
) {
    let open = editor.current_scene_path();
    let Some(active) = &mut editor.dev.lightmap_bake else {
        return;
    };
    if scene.is_playing() {
        // The World holds the play state: wait for Stop's restore.
        return ctx.request_repaint_after(PROGRESS_REPAINT);
    }
    if active.scene != scene.id() || active.path != open {
        // The result's ids would name another scene's meshes, or its lighting land in the
        // wrong file.
        let line = match active.scene != scene.id() {
            true => format!("{} dropped: the open scene changed", verb(&active.step)),
            false => dropped(&active.step, "the scene was saved elsewhere"),
        };
        log.warn(line.clone());
        scene.lighting_bake_status = BakeStatus::Failed(line);
        if let (Step::Lightmaps(job), true) = (&mut active.step, active.scene == scene.id()) {
            scene.unrecorded(|scene| job.restore(scene));
        }
        editor.dev.lightmap_bake = None;
        return;
    }
    loop {
        match advance(&mut active.step, scene, log, nav) {
            Some(needs_save) => {
                editor.dev.lightmap_bake = None;
                editor.is_dirty |= needs_save;
                return;
            }
            None if matches!(active.step, Step::Probes { .. })
                && until.is_some_and(|t| Instant::now() < t) => {}
            None if matches!(active.step, Step::Probes { .. }) => return ctx.request_repaint(),
            None => return ctx.request_repaint_after(PROGRESS_REPAINT),
        }
    }
}

/// `Some` once the bake ended: whether the scene needs a full save.
fn advance(
    step: &mut Step,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) -> Option<bool> {
    let job = match step {
        Step::Lightmaps(job) => job,
        Step::Probes { .. } => return advance_probes(step, scene, log, nav),
        Step::Occlusion { .. } => return advance_occlusion(step, scene, log),
        Step::OcclusionOnly { .. } => return advance_occlusion_only(step, scene, log),
    };
    if !job.is_finished() {
        scene.unrecorded(|scene| job.show_preview(scene));
        return None;
    }
    // A bake is no edit: what it writes never joins an Undo step.
    scene.unrecorded(|scene| land_lightmaps(step, scene, log, nav))
}

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
        Err(e) => fail(scene, log, e, true),
    }
}

/// One probe step; at the end commit the probes, write the lighting, start the PVS.
fn advance_probes(
    step: &mut Step,
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    nav: &NavigationGraph,
) -> Option<bool> {
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
            match OcclusionBakeJob::generate(scene, reach(scene, Some(nav))) {
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
            Some(true)
        }
        Err(e) => fail(scene, log, e, true),
    }
}

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
        Err(e) => fail(scene, log, e, true),
    }
}

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
        Err(e) => fail_occlusion(scene, log, e, true),
    }
}
