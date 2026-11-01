//! src/dev/bake/probe.rs — Headless entry point for the multi-bounce probe bake (#241, #285).
//!
//! The dev-layer bridge between the script-side `Probe.Bake()` action and the
//! render-layer multi-bounce solve (`BounceSolve`, the one `Renderer::bake_probes` runs),
//! stepped here so a `BakeLog` hears each bounce (#1667). Like `screenshot.rs`, it spins up a
//! headless `Renderer` (no window surface) sized for one cubemap face, runs the bake,
//! and skips gracefully when no GPU/software adapter is present — it NEVER panics, so
//! an unattended/headless run with no adapter degrades to a no-op rather than a crash.
//!
//! Authoring-time only: a probe bake is a dev action (the baked SH is saved into the
//! `<scene>.lighting.json` sidecar and loaded by the runtime), so the whole path lives
//! behind the `dev` feature with the rest of the agentic layer.

use super::progress::{stage_progress, BakeLog};
use super::steps::Stage;
use crate::render::ibl::probe::solve::BounceSolve;
use crate::render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use crate::scene::Scene;

/// Bake every probe in `scene` from rendered cubemap captures, writing each probe's
/// L2 SH in place. Returns `Ok(true)` when the bake ran, `Ok(false)` when no
/// GPU/software adapter is available (skipped gracefully, same contract as the
/// screenshot path). With no probes the bake is a successful no-op. Where each bounce
/// is goes to `log` (#1667): the solve is `Renderer::bake_probes`' own, stepped here.
pub fn bake_scene_probes(scene: &mut Scene, log: &mut BakeLog) -> Result<bool, String> {
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
    let mut solve = BounceSolve::new(scene, res);
    loop {
        let (bounce, done, total) = solve.progress();
        if let Some(p) = stage_progress(Stage::Light {
            bounce,
            done,
            total,
        }) {
            log.observe(p);
        }
        if solve.step(&mut renderer, scene).is_some() {
            break;
        }
    }
    solve.commit(scene)?;
    log.finish();
    Ok(true)
}
