use std::collections::BTreeMap;

use crate::editor::EditorFrontend;
use rusty_app::GameWorld;
use rusty_core::fs::written::{written_since, Stale};
use rusty_edit::load_parsed;
use rusty_edit::scenes::reload_loaded;
use rusty_scene::{parse_scene_text, Scene};
use rusty_script::scripting::ConsoleLogs;

pub struct SceneFollow {
    /// The write-log generation already read.
    seen: u64,
    /// Changed files not yet reloaded, one a frame.
    pending: Vec<String>,
    /// Per file, text that failed to load (warned once), not parsed again.
    failed: BTreeMap<String, String>,
}

impl SceneFollow {
    /// Start at the write log's current generation: what came before is loaded.
    pub fn new() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
            pending: Vec::new(),
            failed: BTreeMap::new(),
        }
    }

    /// Each edit-mode frame, after the watch's notes: reload `scene` when one of its files
    /// changed outside the engine. Returns whether it reloaded.
    pub fn apply(
        &mut self,
        scene: &mut Scene,
        path: Option<&str>,
        selected: &mut Option<u32>,
        console: &mut ConsoleLogs,
    ) -> bool {
        if scene.is_playing() {
            return false;
        }
        let (seen, written) = written_since(self.seen);
        self.seen = seen;
        let paths: Vec<String> = match scene.loaded.active_of_several() {
            Some(_) => scene.loaded.paths(None).into_iter().flatten().collect(),
            None => path.map(str::to_string).into_iter().collect(),
        };
        self.pending.retain(|p| paths.contains(p));
        self.failed.retain(|p, _| paths.contains(p));
        let stale = Stale::new(written);
        for changed in paths.iter().filter(|p| stale.matches(p)) {
            if !self.pending.contains(changed) {
                self.pending.push(changed.clone());
            }
            self.failed.remove(changed);
        }
        for path in self.pending.clone() {
            let Ok(on_disk) = std::fs::read_to_string(&path) else {
                continue;
            };
            if scene.is_disk_text(&path, &on_disk) {
                self.pending.retain(|p| *p != path);
                continue;
            }
            if self.failed.get(&path) == Some(&on_disk) {
                continue;
            }
            return self.reload(scene, &path, on_disk, selected, console);
        }
        false
    }

    fn reload(
        &mut self,
        scene: &mut Scene,
        path: &str,
        on_disk: String,
        selected: &mut Option<u32>,
        console: &mut ConsoleLogs,
    ) -> bool {
        let data = match parse_scene_text(&on_disk, path) {
            Ok(data) => data,
            Err(err) => {
                if self.failed.insert(path.to_string(), on_disk).is_none() {
                    console.warn(format!(
                        "Scene {path} changed on disk but did not load: {err}"
                    ));
                }
                return false;
            }
        };
        // Only after the parse: this serialises the live scene, which a broken file must not cost.
        let unsaved = rusty_edit::has_unsaved_edits(scene, path);
        let keep = selected.and_then(|id| name_path(scene, id));
        let reloaded = match scene.loaded.active_of_several() {
            Some(_) => reload_loaded(scene, path, on_disk, data),
            None => load_parsed(scene, path, on_disk, data),
        };
        if let Err(err) = reloaded {
            console.warn(format!("Scene {path}: {err}"));
        }
        self.pending.retain(|p| p != path);
        self.failed.remove(path);
        *selected = keep.and_then(|names| find_by_name_path(scene, &names));
        if unsaved {
            console.warn(format!(
                "Scene reloaded: {path} changed on disk; unsaved edits were discarded"
            ));
        } else {
            console.info(format!("Scene reloaded: {path} changed on disk"));
        }
        true
    }
}

impl EditorFrontend {
    pub(in crate::editor) fn follow_scene_swap(&mut self, game: &GameWorld) {
        let id = game.scene().borrow().id();
        if self.scene_id.is_some_and(|last| last != id) {
            self.editor_ui.selected_entity_id = None;
        }
        self.scene_id = Some(id);
    }

    /// Sets `scene_id`, so `follow_scene_swap` keeps the selection this reload remapped.
    pub(in crate::editor) fn follow_scene_file(&mut self, game: &GameWorld) {
        let ui = &mut self.editor_ui;
        let mut scene = game.world.scene.borrow_mut();
        let mut console = game.resources.console.borrow_mut();
        let path = ui.current_scene_path();
        if self.follow.apply(
            &mut scene,
            path.as_deref(),
            &mut ui.selected_entity_id,
            &mut console,
        ) {
            ui.loaded(&mut scene);
            self.scene_id = Some(scene.id());
        }
    }
}

fn name_path(scene: &Scene, id: u32) -> Option<Vec<String>> {
    let mut names = Vec::new();
    let mut at = Some(id);
    while let Some(id) = at {
        names.push(scene.world.name(id)?.clone());
        at = scene.world.parent_id(id);
    }
    names.reverse();
    Some(names)
}

/// The first entity, in id order, whose name path is `names`.
fn find_by_name_path(scene: &Scene, names: &[String]) -> Option<u32> {
    scene
        .entity_ids()
        .into_iter()
        .find(|&id| name_path(scene, id).as_deref() == Some(names))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod echo_tests;

#[cfg(test)]
mod scenes_tests;
