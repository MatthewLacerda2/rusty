//! The editor's current scene file and its verbs: the File menu's new / reset /
//! load / save, and the one open and one save every other editor surface (the scene
//! asset's inspector buttons) goes through, so they never drift apart.

use std::path::Path;

use crate::EditorUi;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

impl EditorUi {
    pub fn current_scene_path(&self) -> Option<String> {
        self.scene_path.borrow().clone()
    }

    pub fn with_current_scene_path<R>(&self, f: impl FnOnce(Option<&str>) -> R) -> R {
        f(self.scene_path.borrow().as_deref())
    }

    pub fn scene_name(&self) -> String {
        self.with_current_scene_path(|path| {
            path.and_then(|p| Path::new(p).file_stem()).map_or_else(
                || "Untitled".to_string(),
                |s| s.to_string_lossy().into_owned(),
            )
        })
    }

    /// Make `path` the scene file the editor (and `Scene.Save()`) writes back to.
    pub fn set_current_scene_path(&mut self, path: Option<String>) {
        *self.scene_path.borrow_mut() = path;
    }

    pub fn open_scene(
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
        match rusty_edit::load_from_file(scene, path) {
            Ok(()) => console.info(format!("Scene loaded from {path}")),
            Err(err) if err.replaced_world() => console.warn(format!("Scene {path}: {err}")),
            Err(err) => {
                console.error(format!("Failed to load scene: {err}"));
                return false;
            }
        }
        self.set_current_scene_path(Some(path.to_string()));
        self.selected_entity_id = None;
        self.selected_asset_path = None;
        self.is_dirty = false;
        true
    }

    /// Save the scene to `path`, which becomes the current scene file.
    pub(crate) fn save_scene_to(&mut self, scene: &Scene, console: &mut ConsoleLogs, path: &str) {
        match rusty_edit::save_to_file(scene, path) {
            Ok(_) => {
                self.set_current_scene_path(Some(path.to_string()));
                self.is_dirty = false;
                console.info(format!("Scene saved to {path}"));
            }
            Err(err) => console.error(format!("Failed to save scene: {err}")),
        }
    }
}

/// A never-saved scene refuses: falling back to a real file would write an empty scene over it.
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

/// New and Reset drop the World with no `OnDisable`/`OnDestroy`: in Play, script instances would leak.
fn refuse_in_play(console: &mut ConsoleLogs, is_playing: bool, what: &str) -> bool {
    if is_playing {
        console.warn(format!("{what} is unavailable in Play: stop Play first"));
    }
    is_playing
}

pub(super) fn new_scene(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    is_playing: bool,
) {
    if refuse_in_play(console, is_playing, "New Scene") {
        return;
    }
    if let Err(why) = rusty_scene::new_scene(scene, &mut editor.scene_path.borrow_mut()) {
        console.warn(why.to_string());
        return;
    }
    editor.selected_entity_id = None;
    editor.selected_asset_path = None;
    editor.is_dirty = false;
    console.info("New empty scene".to_string());
}

pub(super) fn reset_scene(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    is_playing: bool,
) {
    if refuse_in_play(console, is_playing, "Reset Scene") {
        return;
    }
    let path = rusty_edit::build_default_scene(scene);
    scene.adopt_disk_text(&path);
    editor.set_current_scene_path(Some(path));
    editor.selected_entity_id = None;
    editor.selected_asset_path = None;
    editor.is_dirty = false;
    console.info("Scene reset to default".to_string());
}

#[cfg(test)]
#[path = "scene_io_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scene_path_tests.rs"]
mod scene_path_tests;
