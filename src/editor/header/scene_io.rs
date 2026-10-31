//! The editor's current scene file and its verbs: the File menu's new / reset /
//! load / save, and the one open and one save every other editor surface (the content grid's double-click,
//! the scene asset's inspector buttons) goes through, so they never drift apart (#997).

use std::path::Path;

use crate::editor::EditorUi;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

impl EditorUi {
    /// The scene file the editor is editing, `None` for a scene never saved. One
    /// value with the script runtime's `Scene.Save()` target (#1366).
    pub fn current_scene_path(&self) -> Option<String> {
        self.scene_path.borrow().clone()
    }

    /// Make `path` the scene file the editor (and `Scene.Save()`) writes back to.
    pub fn set_current_scene_path(&mut self, path: Option<String>) {
        *self.scene_path.borrow_mut() = path;
    }

    /// Open the scene file at `path` — the editor face of `Scene.Load`, with its rules:
    /// a path that is not a file is refused, and during play the swap is deferred to
    /// the tick's tail. In edit mode the scene loads at once, becomes the current
    /// scene file, and both selections clear so the inspector shows Scene Settings.
    /// Returns whether the World now holds the file's scene.
    pub(crate) fn open_scene(
        &mut self,
        scene: &mut Scene,
        console: &mut ConsoleLogs,
        path: &str,
        is_playing: bool,
    ) -> bool {
        if !Path::new(path).is_file() {
            console.error(format!("Failed to load scene: no scene file at '{path}'"));
            return false;
        }
        if is_playing {
            scene.request_load(path.to_string());
            console.info(format!("Scene load queued: {path}"));
            return false;
        }
        match scene.load_from_file(path) {
            Ok(()) => console.info(format!("Scene loaded from {path}")),
            // The World is the new file's: so is the current scene file (#1374).
            Err(err) if err.replaced_world() => console.warn(format!("Scene {path}: {err}")),
            Err(err) => {
                console.error(format!("Failed to load scene: {err}"));
                return false;
            }
        }
        self.set_current_scene_path(Some(path.to_string()));
        self.selected_entity_id = None;
        self.selected_asset_path = None;
        self.is_dirty = true;
        true
    }

    /// Save the scene to `path`, which becomes the current scene file.
    pub(crate) fn save_scene_to(&mut self, scene: &Scene, console: &mut ConsoleLogs, path: &str) {
        match scene.save_to_file(path) {
            Ok(_) => {
                self.set_current_scene_path(Some(path.to_string()));
                console.info(format!("Scene saved to {path}"));
            }
            Err(err) => console.error(format!("Failed to save scene: {err}")),
        }
    }
}

/// Save and Load act on the CURRENT scene file (set on load / double-click). A scene
/// never saved (File ▸ New) has none, and the verb is refused: falling back to a
/// real file would write an empty scene over it (#1374). Unity would open Save As;
/// the editor has none yet, so it says so, as `Scene.Save()` does.
fn current_path(editor: &EditorUi, console: &mut ConsoleLogs, verb: &str) -> Option<String> {
    let path = editor.current_scene_path();
    if path.is_none() {
        console.error(format!(
            "{verb}: the scene has no file yet; use a scene asset's Overwrite with Current, or Scene.Save(path)"
        ));
    }
    path
}

pub(super) fn save_scene(editor: &mut EditorUi, scene: &Scene, console: &mut ConsoleLogs) {
    if let Some(path) = current_path(editor, console, "Save Scene") {
        editor.save_scene_to(scene, console, &path);
    }
}

pub(super) fn load_scene(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    is_playing: bool,
) {
    if let Some(path) = current_path(editor, console, "Load Scene") {
        editor.open_scene(scene, console, &path, is_playing);
    }
}

/// New / Reset replace the World outright, with no `OnDisable`/`OnDestroy`, so in
/// Play they would leak every script instance (#1305). Unity's way: not during Play.
/// The menu greys them out; this is the guard for any other caller.
fn refuse_in_play(console: &mut ConsoleLogs, is_playing: bool, what: &str) -> bool {
    if is_playing {
        console.warn(format!("{what} is unavailable in Play: stop Play first"));
    }
    is_playing
}

/// Start a fresh, empty scene — a blank slate (no entities, default ambient/sky/
/// layers), discarding the in-memory scene. Distinct from Reset Scene, which
/// rebuilds the *engine's default* (its ground, lights, etc.). The new scene is
/// unsaved, so its path is cleared until the next Save.
pub(super) fn new_scene(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    is_playing: bool,
) {
    if refuse_in_play(console, is_playing, "New Scene") {
        return;
    }
    scene.clear();
    editor.set_current_scene_path(None);
    editor.selected_entity_id = None;
    editor.selected_asset_path = None;
    editor.is_dirty = true;
    console.info("New empty scene".to_string());
}

/// Rebuild the engine's current default scene (discards the in-memory scene). Built
/// fresh, never loaded from the seeded file, which may be stale or edited (#746).
pub(super) fn reset_scene(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    is_playing: bool,
) {
    if refuse_in_play(console, is_playing, "Reset Scene") {
        return;
    }
    let path = crate::scene::build_default_scene(scene);
    scene.adopt_disk_text(&path);
    editor.set_current_scene_path(Some(path));
    editor.selected_entity_id = None;
    editor.selected_asset_path = None;
    editor.is_dirty = true;
    console.info("Scene reset to default".to_string());
}

#[cfg(test)]
#[path = "scene_io_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scene_path_tests.rs"]
mod scene_path_tests;
