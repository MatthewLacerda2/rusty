//! src/dev/probe_bake.rs — Headless entry point for the full-bounce probe bake (#241).
//!
//! The dev-layer bridge between the script-side `Probe.Bake()` action and the
//! render-layer bake (`Renderer::bake_probes`). Like `screenshot.rs`, it spins up a
//! headless `Renderer` (no window surface) sized for one cubemap face, runs the bake,
//! and skips gracefully when no GPU/software adapter is present — it NEVER panics, so
//! an unattended/headless run with no adapter degrades to a no-op rather than a crash.
//!
//! Authoring-time only: a probe bake is a dev action (the baked SH is saved into the
//! `<scene>.lighting.json` sidecar and loaded by the runtime), so the whole path lives
//! behind the `dev` feature with the rest of the agentic layer.
//!
//! Allowed deps: render (headless path), scene.

use crate::render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use crate::scene::Scene;

/// Bake every probe in `scene` from rendered cubemap captures, writing each probe's
/// L2 SH in place. Returns `Ok(true)` when the bake ran, `Ok(false)` when no
/// GPU/software adapter is available (skipped gracefully, same contract as the
/// screenshot path). With no probes the bake is a successful no-op.
pub fn bake_scene_probes(scene: &mut Scene) -> Result<bool, String> {
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
    renderer.bake_probes(scene, res);
    Ok(true)
}
