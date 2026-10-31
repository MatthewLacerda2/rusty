//! src/core/project/boot.rs — which scene a binary opens the project on (#1618).
//!
//! Unity's order: the editor reopens the last scene this machine had open for the
//! project (kept in `Library/`, so machine-local), falls back to the build's first
//! scene, and opens an untitled empty scene when neither exists. A run (the player,
//! `play`) boots the build's startup scene, as Unity's build boots scene 0, never
//! the editor's last one. [`boot_scene`] is the one place that order lives, so the
//! editor, `session`, `session-mcp` and `play` can't drift apart again (#1343 fixed
//! `play` alone).

use std::path::Path;

use super::Access;
use crate::core::paths;

/// Where the editor remembers the last scene it had open, relative to the project
/// root: inside `cache/` (Unity's `Library/`), gitignored, never in `project.rusty`.
pub const LAST_SCENE_PATH: &str = "cache/last_scene";

/// The scenes a boot tries, in order, relative to the project root as the project
/// stores them: the first that loads is opened, and when none does (or the list is
/// empty) an untitled empty scene is. `warnings` are for the console.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BootPlan {
    pub scenes: Vec<String>,
    pub warnings: Vec<String>,
}

impl BootPlan {
    /// The plan that boots `scene` alone; an empty `scene` boots an empty one.
    pub fn of(scene: &str) -> Self {
        let scenes = (!scene.is_empty()).then(|| scene.to_string());
        let scenes = scenes.into_iter().collect();
        Self {
            scenes,
            warnings: Vec::new(),
        }
    }
}

/// The scenes the project at `root` boots on. [`Access::Edit`] (the editor and the
/// agent sessions): the remembered scene, then `startup_scene`, each only if its
/// file exists; a named `startup_scene` with no file is skipped with a warning.
/// [`Access::Run`]: `startup_scene` alone, as named, so a run of a missing scene
/// fails loudly instead of playing nothing; `Err` when it names none.
pub fn boot_scene(root: &Path, startup_scene: &str, access: Access) -> Result<BootPlan, String> {
    if access == Access::Run {
        if startup_scene.trim().is_empty() {
            return Err("no startup scene is set (project.rusty, build.startup_scene)".into());
        }
        return Ok(BootPlan::of(startup_scene));
    }
    let exists = |path: &str| !path.is_empty() && root.join(paths::native(path)).is_file();
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

/// What the editor records for its open scene `open`, the project at `root`:
/// the path relative to the root, `/`-spelled. `None` for an Untitled scene, one
/// in Play (the game's, not the editor's), one equal to `last` (already recorded)
/// and one outside the project, which the next launch couldn't name.
pub fn scene_to_remember(
    root: &Path,
    open: Option<&str>,
    last: Option<&str>,
    playing: bool,
) -> Option<String> {
    let open = open.filter(|open| !playing && Some(*open) != last && !open.is_empty())?;
    let relative = paths::relativize(open, root);
    let outside =
        Path::new(&relative).is_absolute() || relative.split(['/', '\\']).any(|c| c == "..");
    (!outside).then(|| paths::stored(&relative))
}

/// Remember `scene` as the last one the editor had open in the project at `root`.
/// Written only when it changed.
pub fn remember_scene(root: &Path, scene: &str) -> Result<(), String> {
    if remembered(root).as_deref() == Some(scene) {
        return Ok(());
    }
    let path = root.join(paths::native(LAST_SCENE_PATH));
    let parent = path.parent().unwrap_or(root);
    std::fs::create_dir_all(parent)
        .and_then(|()| crate::core::fs::write_atomic(&path, scene.as_bytes()))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The remembered scene, if the record exists and names one.
fn remembered(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(paths::native(LAST_SCENE_PATH))).ok()?;
    let scene = text.trim();
    (!scene.is_empty()).then(|| scene.to_string())
}

#[cfg(test)]
#[path = "boot_tests.rs"]
mod tests;
