use crate::editor::EditorFrontend;
use rusty_app::GameWorld;
use rusty_core::fs::written::{written_since, Stale};
use rusty_edit::load_parsed;
use rusty_scene::{parse_scene_text, Scene};
use rusty_script::scripting::ConsoleLogs;

pub struct SceneFollow {
    /// The write-log generation already read.
    seen: u64,
    pending: Option<String>,
    /// Text that failed to load, not parsed again; `Some` also means the warning was logged.
    failed: Option<String>,
}

impl SceneFollow {
    /// Start at the write log's current generation: what came before is loaded.
    pub fn new() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
            pending: None,
            failed: None,
        }
    }

    /// Each edit-mode frame, after the watch's notes: reload `scene` when its
    /// file changed outside the engine. Returns whether it reloaded.
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
        let Some(path) = path else {
            self.pending = None;
            return false;
        };
        if Stale::new(written).matches(path) {
            self.pending = Some(path.to_string());
            self.failed = None;
        }
        if self.pending.as_deref() != Some(path) {
            return false;
        }
        let Ok(on_disk) = std::fs::read_to_string(path) else {
            // Deleted or mid-write: still pending, so read again next frame.
            return false;
        };
        if scene.is_disk_text(path, &on_disk) {
            self.pending = None;
            return false;
        }
        if self.failed.as_deref() == Some(on_disk.as_str()) {
            return false;
        }
        self.reload(scene, path, on_disk, selected, console)
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
                if self.failed.replace(on_disk).is_none() {
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
        if let Err(err) = load_parsed(scene, path, on_disk, data) {
            console.warn(format!("Scene {path}: {err}"));
        }
        self.pending = None;
        self.failed = None;
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
#[path = "follow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "follow_echo_tests.rs"]
mod echo_tests;
