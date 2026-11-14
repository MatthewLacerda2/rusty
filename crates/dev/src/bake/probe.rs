//! crates/dev/src/bake/probe.rs — Headless entry point for the multi-bounce probe bake (#241, #285).
//!
//! The dev-layer bridge between the script-side `Probe.Bake()` action and the
//! render-layer bake (`Renderer::bake_probes_observed`, the multi-bounce solve's one
//! run-to-end loop), whose observer feeds a `BakeLog` (#1667). Like `screenshot.rs`, it spins up a
//! headless `Renderer` (no window surface) sized for one cubemap face, runs the bake,
//! and skips gracefully when no GPU/software adapter is present — it NEVER panics, so
//! an unattended/headless run with no adapter degrades to a no-op rather than a crash.
//!
//! Authoring-time only: a probe bake is a dev action (the baked SH is saved into the
//! `<scene>.lighting.json` sidecar and loaded by the runtime), so the whole path lives
//! behind the `dev` feature with the rest of the agentic layer.

use super::progress::{stage_progress, BakeLog};
use super::report::{self, probes::Timeline};
use super::steps::Stage;
use rusty_render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use rusty_scene::Scene;

/// Bake every probe in `scene` from rendered cubemap captures, writing each probe's
/// L2 SH in place. Returns `Ok(true)` when the bake ran, `Ok(false)` when no
/// GPU/software adapter is available (skipped gracefully, same contract as the
/// screenshot path). With no probes the bake is a successful no-op. Where each bounce
/// is goes to `log` (#1667).
pub fn bake_scene_probes(scene: &mut Scene, log: &mut BakeLog) -> Result<bool, String> {
    bake_scene_probes_at(scene, None, log)
}

/// [`bake_scene_probes`] of the scene saved at `scene_path`: a bake that ran writes
/// its report beside it (`report::probes`, #1980; `Probe.Bake()`).
pub fn bake_scene_probes_at(
    scene: &mut Scene,
    scene_path: Option<&str>,
    log: &mut BakeLog,
) -> Result<bool, String> {
    if scene.probes.is_empty() {
        return Ok(true);
    }
    let res = DEFAULT_BAKE_RESOLUTION;
    let mut renderer = match pollster::block_on(Renderer::new_headless(res, res)) {
        Some(r) => r,
        None => {
            log::warn!("[ProbeBake] no GPU/software adapter available — skipping bake");
            return Ok(false);
        }
    };
    let mut times = Timeline::default();
    renderer.bake_probes_observed(scene, res, &mut |bounce, done, total| {
        let stage = Stage::Light {
            bounce,
            done,
            total,
        };
        times.note(stage);
        if let Some(p) = stage_progress(stage) {
            log.observe(p);
        }
    });
    times.note(Stage::Done);
    log.finish();
    if let Some(path) = scene_path {
        let print = report::probes::scene_fingerprint(scene);
        report::keep(
            path,
            report::probes::report(scene, &times.stages().0, print),
        );
    }
    Ok(true)
}
