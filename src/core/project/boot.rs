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

/// Where the editor remembers the last scene it had open, relative to the project
/// root: inside `cache/` (Unity's `Library/`), gitignored, never in `project.rusty`.
pub const LAST_SCENE_PATH: &str = "cache/last_scene";

/// The scene the project at `root` opens on, relative to the root as the project
/// stores it; empty means an untitled empty scene. [`Access::Edit`] (the editor and
/// the agent sessions): the remembered scene, then `startup_scene`, each only if
/// its file exists. [`Access::Run`]: `startup_scene` as named, so a run of a missing
/// scene fails loudly instead of playing nothing.
pub fn boot_scene(root: &Path, startup_scene: &str, access: Access) -> String {
    if access == Access::Run {
        return startup_scene.to_string();
    }
    let exists = |path: &str| !path.is_empty() && root.join(path).is_file();
    remembered(root)
        .filter(|path| exists(path))
        .or_else(|| exists(startup_scene).then(|| startup_scene.to_string()))
        .unwrap_or_default()
}

/// Remember `scene` as the last one the editor had open in the project at `root`.
/// Written only when it changed.
pub fn remember_scene(root: &Path, scene: &str) -> Result<(), String> {
    if remembered(root).as_deref() == Some(scene) {
        return Ok(());
    }
    let path = root.join(crate::core::paths::native(LAST_SCENE_PATH));
    let parent = path.parent().unwrap_or(root);
    std::fs::create_dir_all(parent)
        .and_then(|()| crate::core::fs::write_atomic(&path, scene.as_bytes()))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The remembered scene, if the record exists and names one.
fn remembered(root: &Path) -> Option<String> {
    let text =
        std::fs::read_to_string(root.join(crate::core::paths::native(LAST_SCENE_PATH))).ok()?;
    let scene = text.trim();
    (!scene.is_empty()).then(|| scene.to_string())
}

#[cfg(test)]
#[path = "boot_tests.rs"]
mod tests;
