//! stored-format: the reflection probes' KTX2 cubemaps in `<scene>.reflections/`

use std::path::{Path, PathBuf};

use super::progress::{stage_progress, BakeLog};
use super::report::{self, probes::Timeline};
use super::steps::Stage;
use rusty_render::ibl::reflection_bake::CUBE_PREFIX;
use rusty_render::Renderer;
use rusty_scene::Scene;

const REFLECTION_DIR_SUFFIX: &str = ".reflections";

pub(super) const UNSAVED: &str = "save the scene before baking reflections";

pub fn reflection_dir(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{REFLECTION_DIR_SUFFIX}"))
}

/// Called only once a cube is about to be written, so a skipped bake leaves no empty folder.
pub(super) fn out_dir(scene_path: Option<&str>) -> Result<PathBuf, String> {
    let dir = reflection_dir(scene_path.ok_or(UNSAVED)?);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("reflection folder {}: {e}", dir.display()))?;
    Ok(dir)
}

pub(super) fn prune_stale(dir: &Path, keep: usize) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
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

pub fn bake_scene_reflections(
    scene: &mut Scene,
    scene_path: Option<&str>,
    log: &mut BakeLog,
) -> Result<bool, String> {
    if scene.reflection_probes.is_empty() {
        return match scene_path {
            Some(path) => prune_stale(&reflection_dir(path), 0).map(|()| true),
            None => Ok(true),
        };
    }
    scene_path.ok_or(UNSAVED)?;

    // Each probe renders into a target of its own size: this one is a placeholder.
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
