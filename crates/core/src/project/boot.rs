use std::path::Path;

use super::Access;
use crate::paths;

/// Where the editor remembers the last scene it had open, relative to the project
/// root: inside `cache/` (Unity's `Library/`), gitignored, never in `project.rusty`.
pub const LAST_SCENE_PATH: &str = "cache/last_scene";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct BootPlan {
    pub scenes: Vec<String>,
    pub warnings: Vec<String>,
}

impl BootPlan {
    pub fn of(scene: &str) -> Self {
        let scenes = (!scene.is_empty()).then(|| scene.to_string());
        let scenes = scenes.into_iter().collect();
        Self {
            scenes,
            warnings: Vec::new(),
        }
    }
}

pub fn boot_scene(root: &Path, startup_scene: &str, access: Access) -> Result<BootPlan, String> {
    if access == Access::Run {
        if startup_scene.trim().is_empty() {
            return Err("no startup scene is set (project.rusty, build.startup_scene)".into());
        }
        return Ok(BootPlan::of(startup_scene));
    }
    let exists = |path: &str| !path.is_empty() && root.join(path).is_file();
    let mut plan = BootPlan::default();
    plan.scenes
        .extend(remembered(root).filter(|path| exists(path)));
    if exists(startup_scene) {
        if !plan.scenes.iter().any(|scene| scene == startup_scene) {
            plan.scenes.push(startup_scene.to_string());
        }
    } else if !startup_scene.trim().is_empty() {
        let warning = format!("the startup scene {startup_scene} is missing; skipped it");
        plan.warnings.push(warning);
    }
    Ok(plan)
}

pub fn scene_to_remember(
    root: &Path,
    open: Option<&str>,
    last: Option<&str>,
    playing: bool,
) -> Option<String> {
    let open = open.filter(|open| !playing && Some(*open) != last && !open.is_empty())?;
    let relative = paths::relativize(open, root);
    let outside = Path::new(&relative).is_absolute() || relative.split('/').any(|c| c == "..");
    (!outside).then(|| paths::stored(&relative))
}

pub fn remember_scene(root: &Path, scene: &str) -> Result<(), String> {
    if remembered(root).as_deref() == Some(scene) {
        return Ok(());
    }
    let path = root.join(LAST_SCENE_PATH);
    let parent = path.parent().unwrap_or(root);
    std::fs::create_dir_all(parent)
        // lint: allow(asset-write) the last-open-scene record, read once on boot
        .and_then(|()| crate::fs::write_atomic(&path, scene.as_bytes()))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn remembered(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(LAST_SCENE_PATH)).ok()?;
    let scene = text.trim();
    (!scene.is_empty()).then(|| scene.to_string())
}

#[cfg(test)]
#[path = "boot_tests.rs"]
mod tests;
