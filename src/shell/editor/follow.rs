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
//! - **Which changes.** The #1327 watch notes every outside change in
//!   `core::fs::written`; this reads that log for the current scene path. A
//!   `.lighting.json` sidecar changing alone is not the scene.
//! - **The editor's own Save is no change.** A save writes the file the watch then
//!   reports; a file whose text is what the live scene serialises to is skipped.
//! - **"Unsaved" is computed, not a flag**: the live scene serialises differently
//!   from the document as last loaded (or saved). It only picks the console line.
//! - **Held through Play**, like every #1327 note: the log is read only in edit
//!   mode, so a change seen in Play applies after Stop, against the restored scene.

use super::EditorFrontend;
use crate::app::GameWorld;
use crate::core::fs::written::{written_since, Stale};
use crate::scene::{scene_json, Scene, SceneId};
use crate::scripting::ConsoleLogs;

/// What the follow needs to remember between frames.
pub struct SceneFollow {
    /// The write-log generation already read.
    seen: u64,
    /// The scene document as last loaded or saved, by the scene it belongs to.
    baseline: Option<(SceneId, String)>,
}

impl SceneFollow {
    /// Start at the write log's current generation: what came before is loaded.
    pub fn new() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
            baseline: None,
        }
    }

    /// Once per edit-mode frame, after the watch noted its changes: reload `scene`
    /// from `path` when that file changed to something else than the live scene.
    /// `selected` is kept by name path. Returns whether the scene was reloaded.
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
        if self
            .baseline
            .as_ref()
            .is_none_or(|(id, _)| *id != scene.id())
        {
            self.baseline = scene_json(scene).ok().map(|doc| (scene.id(), doc));
        }
        let (seen, written) = written_since(self.seen);
        self.seen = seen;
        let Some(path) = path.filter(|p| Stale::new(written).matches(p)) else {
            return false;
        };
        let Ok(on_disk) = std::fs::read_to_string(path) else {
            return false; // deleted or mid-write: keep the working copy
        };
        let live = scene_json(scene).ok();
        if live.as_deref() == Some(on_disk.as_str()) {
            self.baseline = live.map(|doc| (scene.id(), doc)); // a save's own echo
            return false;
        }
        let unsaved = self.baseline.as_ref().map(|(_, doc)| doc) != live.as_ref();
        let keep = selected.and_then(|id| name_path(scene, id));
        if let Err(err) = scene.load_from_file(path) {
            console.warn(format!(
                "Scene {path} changed on disk but did not load: {err}"
            ));
            return false;
        }
        *selected = keep.and_then(|names| find_by_name_path(scene, &names));
        self.baseline = scene_json(scene).ok().map(|doc| (scene.id(), doc));
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
    /// Reload the open scene when its file changed on disk (#1340). The reload is
    /// the editor's own, so it keeps the selection `follow_scene_swap` would drop.
    pub(super) fn follow_scene_file(&mut self, game: &GameWorld) {
        let ui = &mut self.editor_ui;
        let mut scene = game.world.scene.borrow_mut();
        let mut console = game.resources.console.borrow_mut();
        let path = ui.current_scene_path.as_deref();
        if self
            .follow
            .apply(&mut scene, path, &mut ui.selected_entity_id, &mut console)
        {
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
