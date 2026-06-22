//! src/dev/reflection_bake.rs — Headless entry point for the reflection-probe bake (#245).
//!
//! The dev-layer bridge between the script-side `Reflection.Bake()` action and the
//! render-layer bake (`Renderer::bake_reflections`), the specular sibling of
//! `probe_bake.rs`. It spins up a headless `Renderer` (no window surface) sized for one
//! cubemap face, captures + GGX-prefilters each probe, writes the KTX2 cubemaps next to
//! the scene, and points each probe at its file. Like the light-probe bake it skips
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
//!
//! Allowed deps: render (headless path), scene.

use std::path::Path;

use crate::render::{Renderer, DEFAULT_REFLECTION_RESOLUTION};
use crate::scene::Scene;

/// Bake every reflection probe in `scene`, writing each one's KTX2 cubemap next to
/// `scene_path` and pointing the probe at it. Returns `Ok(true)` when the bake ran,
/// `Ok(false)` when no GPU/software adapter is available (skipped gracefully). With no
/// probes the bake is a successful no-op. Errors only when there is no saved scene path
/// to write beside, or a cubemap file could not be written.
pub fn bake_scene_reflections(scene: &mut Scene, scene_path: Option<&str>) -> Result<bool, String> {
    if scene.reflection_probes.is_empty() {
        return Ok(true);
    }
    let scene_path =
        scene_path.ok_or_else(|| "save the scene before baking reflections".to_string())?;
    let out_dir = Path::new(scene_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let res = DEFAULT_REFLECTION_RESOLUTION;
    let mut renderer = match pollster::block_on(Renderer::new_headless(res, res)) {
        Some(r) => r,
        None => {
            log::warn!("[ReflectionBake] no GPU/software adapter available — skipping bake");
            return Ok(false);
        }
    };
    renderer.bake_reflections(scene, &out_dir, res)?;
    Ok(true)
}
