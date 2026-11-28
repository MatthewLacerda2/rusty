use rusty_render::ClockProbe;

use super::super::super::harness::Harness;
use super::super::{clock, Report};
use super::views::{self, Viewpoint};
use super::{fold, Options, SceneReport};

/// `cameras`: the scene's authored viewpoints, `Load::cameras`.
pub(super) fn all(
    harness: &mut Harness,
    probe: Option<&ClockProbe>,
    cameras: Vec<u32>,
    opts: Options,
) -> SceneReport {
    let views = {
        let world = harness.world.borrow();
        let scene = world.scene().borrow();
        let nav = world.resources.nav.borrow();
        views::pick_from(&scene, &nav, cameras)
    };
    let mut runs = Vec::new();
    let mut spreads = Vec::new();
    let (mut measured, mut quit) = (0, None);
    for run in 1..=opts.runs.max(1) {
        let mut round = Vec::new();
        for viewpoint in &views.views {
            let (report, samples) = one(harness, probe, viewpoint, opts);
            measured += report.frames;
            if report.frames < opts.frames && quit.is_none() {
                quit = Some(format!("{} (run {run})", viewpoint.label));
            }
            let window = match opts.runs > 1 {
                true => format!("{} run {run}", viewpoint.label),
                false => viewpoint.label.clone(),
            };
            spreads.push((window, clock::cpu_spreads(&samples)));
            round.push((viewpoint.label.clone(), report));
        }
        runs.push(fold::aggregate(&round));
    }
    let mut warnings = views.warnings;
    warnings.extend(fold::spread_warnings(&spreads, clock::GAME_ROWS));
    let planned = opts.runs.max(1) * views.views.len() as u32 * opts.frames;
    warnings.extend(quit.map(|at| quit_warning(&at, measured, planned)));
    SceneReport {
        views: views.views.iter().map(Viewpoint::key).collect(),
        frames: opts.frames,
        runs: opts.runs.max(1),
        measured,
        rows: fold::over_runs(&runs),
        warnings,
        ..SceneReport::default()
    }
}

fn one(
    harness: &mut Harness,
    probe: Option<&ClockProbe>,
    viewpoint: &Viewpoint,
    opts: Options,
) -> (Report, super::super::Samples) {
    one_with(harness, probe, viewpoint, opts, &mut || {})
}

pub(super) fn one_with(
    harness: &mut Harness,
    probe: Option<&ClockProbe>,
    viewpoint: &Viewpoint,
    opts: Options,
    warmed: &mut dyn FnMut(),
) -> (Report, super::super::Samples) {
    let mut switched = views::Switched::default();
    let mut pose = |h: &mut Harness| {
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        let mut camera = world.resources.camera.borrow_mut();
        views::apply(&mut scene, &mut camera, &viewpoint.view, &mut switched);
    };
    warm(harness, opts.warmup, &mut pose);
    warmed();
    let measured = super::super::measure_posed(harness, opts.frames, probe, &mut pose);
    let world = harness.world.borrow();
    views::restore(&mut world.scene().borrow_mut(), switched);
    measured
}

fn warm(harness: &mut Harness, frames: u32, pose: &mut dyn FnMut(&mut Harness)) {
    for _ in 0..frames {
        harness.step(1);
        pose(harness);
        harness.render_frame();
    }
}

fn quit_warning(at: &str, measured: u32, planned: u32) -> String {
    format!(
        "the game called Application.Quit() at {at}: {measured} of {planned} frames \
         measured, and the viewpoints after it measured none (the rows are over the \
         frames measured)"
    )
}
