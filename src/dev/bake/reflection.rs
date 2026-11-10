//! src/dev/bake/reflection.rs — Headless entry point for the reflection-probe bake (#245).
//!
//! The dev-layer bridge between the script-side `Reflection.Bake()` action and the
//! render-layer bake (`Renderer::bake_reflections_observed`), the specular sibling of
//! `probe.rs`. It spins up a headless `Renderer` (no window surface),
//! captures and GGX-prefilters each probe at its resolution, writes the KTX2 cubemaps
//! into the scene's own folder, and points each probe at its file. Like the light-probe bake it skips
//! gracefully when no GPU/software adapter is present — it NEVER panics, so an unattended
//! headless run with no adapter degrades to a no-op rather than a crash.
//!
//! The cubemaps are written to the scene's own folder, `<scene file>.reflections/`
//! ([`reflection_dir`], #1692), as its lightmaps go to `<scene file>.lightmaps/`, so two
//! scenes in one folder never bake over each other's cubes. A bake removes the cubes
//! its own folder holds past its probe count (`prune_stale`) and touches nothing
//! outside it. Without a saved scene path there is nowhere to put them, so the bake
//! reports an error the script surface surfaces — the one case it does not silently
//! no-op.
//!
//! Authoring-time only: a reflection bake is a dev action (its output is baked-asset
//! files + path refs the runtime loads), so the whole path lives behind the `dev` feature
//! with the rest of the agentic layer.
//!
//! A bake that ran writes its report beside the scene (`report::reflections`,
//! #1980).
//!
//! stored-format: the reflection probes' KTX2 cubemaps in `<scene>.reflections/`

use std::path::{Path, PathBuf};

use super::progress::{stage_progress, BakeLog};
use super::report::{self, probes::Timeline};
use super::steps::Stage;
use crate::render::ibl::reflection_bake::CUBE_PREFIX;
use crate::render::Renderer;
use crate::scene::Scene;

/// Suffix of the folder beside a scene file that holds its reflection cubes.
const REFLECTION_DIR_SUFFIX: &str = ".reflections";

/// Why a reflection bake cannot start: there is nowhere to write its cubes.
pub(super) const UNSAVED: &str = "save the scene before baking reflections";

/// The reflection-cube folder for a scene: `foo.scene` → `foo.scene.reflections/`.
pub fn reflection_dir(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{REFLECTION_DIR_SUFFIX}"))
}

/// [`reflection_dir`] of a saved scene, created; an error when the scene is unsaved.
/// Called only once a cube is about to be written, so a bake that skips (no adapter)
/// leaves no empty folder behind to ship in an export.
pub(super) fn out_dir(scene_path: Option<&str>) -> Result<PathBuf, String> {
    let dir = reflection_dir(scene_path.ok_or(UNSAVED)?);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("reflection folder {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Remove the cubes a previous bake left in `dir` past the first `keep`
/// (`reflection_probe_<i>.ktx2` with `i >= keep`), as a lightmap rebake empties its
/// folder of old pages. Nothing else in `dir` is touched; a missing `dir` is fine.
pub(super) fn prune_stale(dir: &Path, keep: usize) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(()); // never baked
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let index = name
            .strip_prefix(CUBE_PREFIX)
            .and_then(|rest| rest.strip_suffix(".ktx2"))
            .and_then(|i| i.parse::<usize>().ok());
        if index.is_some_and(|i| i >= keep) {
            std::fs::remove_file(entry.path()).map_err(|e| format!("remove {name}: {e}"))?;
        }
    }
    Ok(())
}

/// Bake every reflection probe in `scene`, writing each one's KTX2 cubemap into
/// `scene_path`'s [`reflection_dir`] and pointing the probe at it, then remove the
/// folder's cubes past the probe count. Returns `Ok(true)` when the bake ran,
/// `Ok(false)` when no GPU/software adapter is available (skipped gracefully: nothing
/// is written or removed). **With no probes it is not a no-op:** it deletes every cube
/// in the scene's folder (none of them is any probe's any more) and returns `Ok(true)`
/// without needing an adapter; an unsaved scene with no probes does nothing. Errors
/// when there are probes and no saved scene path to write into, a cubemap file could
/// not be written, or a stale cube could not be removed. Each probe is reported to
/// `log` (#1667).
pub fn bake_scene_reflections(
    scene: &mut Scene,
    scene_path: Option<&str>,
    log: &mut BakeLog,
) -> Result<bool, String> {
    if scene.reflection_probes.is_empty() {
        // No probes, so none of the folder's cubes is anyone's any more.
        return match scene_path {
            Some(path) => prune_stale(&reflection_dir(path), 0).map(|()| true),
            None => Ok(true),
        };
    }
    scene_path.ok_or(UNSAVED)?; // refused before anything is captured

    // Each probe's faces render into a target of their own size, so the renderer's
    // own is only a placeholder.
    let mut renderer = match pollster::block_on(Renderer::new_headless(64, 64)) {
        Some(r) => r,
        None => {
            log::warn!("[ReflectionBake] no GPU/software adapter available — skipping bake");
            return Ok(false);
        }
    };
    let out_dir = out_dir(scene_path)?;
    let mut times = Timeline::default();
    let baked = renderer.bake_reflections_observed(scene, &out_dir, &mut |done, total| {
        times.note(Stage::Reflection { done, total });
        if let Some(p) = stage_progress(Stage::Reflection { done, total }) {
            log.observe(p);
        }
    })?;
    times.note(Stage::Done);
    prune_stale(&out_dir, baked)?;
    log.finish();
    if let Some(path) = scene_path {
        let (stages, times) = (times.stages().1, &times.reflections);
        let print = report::probes::scene_fingerprint(scene);
        let report = report::reflections::report(scene, (&stages, times), &out_dir, print);
        report::keep(path, report);
    }
    Ok(true)
}

#[cfg(test)]
#[path = "reflection_rebake_tests.rs"]
mod rebake_tests;
#[cfg(test)]
#[path = "reflection_tests.rs"]
mod tests;
