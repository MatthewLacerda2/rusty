//! src/dev/bake/reflection.rs — Headless entry point for the reflection-probe bake (#245).
//!
//! The dev-layer bridge between the script-side `Reflection.Bake()` action and the
//! render-layer bake (`Renderer::bake_reflection_into`, probe by probe), the specular sibling of
//! `probe.rs`. It spins up a headless `Renderer` (no window surface),
//! captures and GGX-prefilters each probe at its resolution, writes the KTX2 cubemaps
//! next to the scene, and points each probe at its file. Like the light-probe bake it skips
//! gracefully when no GPU/software adapter is present — it NEVER panics, so an unattended
//! headless run with no adapter degrades to a no-op rather than a crash.
//!
//! The cubemaps are written beside the scene file (in its parent directory). Without a
//! saved scene path there is nowhere to put them, so the bake reports an error the
//! script surface surfaces — the one case it does not silently no-op.
//!
//! Authoring-time only: a reflection bake is a dev action (its output is baked-asset
//! files + path refs the runtime loads), so the whole path lives behind the `dev` feature
//! with the rest of the agentic layer.

use std::path::Path;

use super::progress::{stage_progress, BakeLog};
use super::steps::Stage;
use crate::render::Renderer;
use crate::scene::Scene;

/// Bake every reflection probe in `scene`, writing each one's KTX2 cubemap next to
/// `scene_path` and pointing the probe at it. Returns `Ok(true)` when the bake ran,
/// `Ok(false)` when no GPU/software adapter is available (skipped gracefully). With no
/// probes the bake is a successful no-op. Errors only when there is no saved scene path
/// to write beside, or a cubemap file could not be written. Each probe is reported to
/// `log` (#1667).
pub fn bake_scene_reflections(
    scene: &mut Scene,
    scene_path: Option<&str>,
    log: &mut BakeLog,
) -> Result<bool, String> {
    if scene.reflection_probes.is_empty() {
        return Ok(true);
    }
    let scene_path =
        scene_path.ok_or_else(|| "save the scene before baking reflections".to_string())?;
    let out_dir = Path::new(scene_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    // Each probe's faces render into a target of their own size, so the renderer's
    // own is only a placeholder.
    let mut renderer = match pollster::block_on(Renderer::new_headless(64, 64)) {
        Some(r) => r,
        None => {
            log::warn!("[ReflectionBake] no GPU/software adapter available — skipping bake");
            return Ok(false);
        }
    };
    // `Renderer::bake_reflections`' loop, one probe at a time so `log` sees each (#1667).
    let total = scene.reflection_probes.len();
    for i in 0..total {
        if let Some(p) = stage_progress(Stage::Reflection { done: i, total }) {
            log.observe(p);
        }
        renderer.bake_reflection_into(scene, i, &out_dir)?;
    }
    log.finish();
    Ok(true)
}
