//! The open scene follows its file on disk (#1340).
//!
//! **The file on disk is the authority; the editor's copy is a working copy.** An
//! agent or a text editor changes a `.scene` on disk at once; the editor's edits
//! reach disk only on Save. So when the open scene's file changes outside the
//! editor, the file wins, always: the scene reloads, the selection is kept by its
//! entity's name path where that still exists, and the console says so. Unsaved
//! edits are discarded with a warning, never a dialog, so an agent editing the file
//! is never blocked (the operator, 2026-10-24; Unity would ask).
//!
//! - **Which changes.** The watch notes every outside change in `core::fs::written`;
//!   this reads that log for the current scene path. A `.lighting.json` sidecar
//!   changing alone is not the scene.
//! - **The engine's own writes are no change.** A file holding exactly the text the
//!   engine last read or wrote there (`Scene::is_disk_text`) is skipped: the echo of
//!   a Save, Generate Lighting rewriting its fields (whose unsaved edits stay
//!   unsaved, #832), a `touch`, a rescan, the same bytes written again.
//! - **"Unsaved" is computed, not a flag** (`Scene::has_unsaved_edits`), and only
//!   picks the console line: info, or a warning that edits were discarded.
//! - **A half-written file is retried.** A read or parse failure keeps the path
//!   pending, so the next frame reads it again, until it loads or a newer note comes.
//!   The file is parsed before anything else, and text that already failed is not
//!   parsed again, so a broken file left on disk costs one read a frame (#1374).
//! - **Held through Play**, like every watch note: the log is read only in edit
//!   mode, so a change seen in Play applies after Stop, against the restored scene.

use crate::app::GameWorld;
use crate::core::fs::written::{written_since, Stale};
use crate::scene::{load_parsed, parse_scene_text, Scene};
use crate::scripting::ConsoleLogs;
use crate::shell::editor::EditorFrontend;

/// What the follow needs to remember between frames.
pub struct SceneFollow {
    /// The write-log generation already read.
    seen: u64,
    /// The scene file noted changed and not yet read whole.
    pending: Option<String>,
    /// The pending file's text that last failed to load, so the same bytes are not
    /// parsed again every frame (#1374); `Some` also means the failure was logged.
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

    /// Once per edit-mode frame, after the watch noted its changes: reload `scene`
    /// from `path` when that file changed outside the engine. `selected` is kept by
    /// name path. Returns whether the scene was reloaded.
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
            return false; // deleted or mid-write: keep the working copy, read again
        };
        if scene.is_disk_text(path, &on_disk) {
            self.pending = None;
            return false;
        }
        if self.failed.as_deref() == Some(on_disk.as_str()) {
            return false; // the same bytes failed last frame: wait for new ones
        }
        self.reload(scene, path, on_disk, selected, console)
    }

    /// Replace `scene` with `on_disk`, the changed text of `path`, and say so.
    /// Parse first: the live scene is serialised (for `unsaved`) only once the file
    /// is known to load, so a broken Dust2-sized file costs one parse.
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
        let unsaved = scene.has_unsaved_edits(path);
        let keep = selected.and_then(|id| name_path(scene, id));
        // Only the sidecar can fail now, after the World was replaced: it is still a
        // reload, with its selection remapped, and the error is a warning (#1374).
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
    /// A different scene now sits in the World (its identity changed without the
    /// editor opening it): as File ▸ Open does, drop the selection, which names an
    /// entity of the old scene.
    pub(in crate::shell::editor) fn follow_scene_swap(&mut self, game: &GameWorld) {
        let id = game.scene().borrow().id();
        if self.scene_id.is_some_and(|last| last != id) {
            self.editor_ui.selected_entity_id = None;
        }
        self.scene_id = Some(id);
    }

    /// Reload the open scene when its file changed on disk (#1340). The reload is
    /// the editor's own, so it keeps the selection `follow_scene_swap` would drop.
    pub(in crate::shell::editor) fn follow_scene_file(&mut self, game: &GameWorld) {
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
            ui.is_dirty = true;
            self.scene_id = Some(scene.id());
        }
    }
}

/// The names from the root down to `id`, or `None` for a dead id.
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
