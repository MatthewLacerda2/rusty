//! The editor's current scene file, one value with the script runtime's
//! `Scene.Save()` target (#1366).

use std::cell::RefCell;
use std::rc::Rc;

use super::EditorUi;

impl EditorUi {
    /// The scene file the editor is editing, `None` for a scene never saved.
    pub fn current_scene_path(&self) -> Option<String> {
        self.scene_path.borrow().clone()
    }

    /// Make `path` the scene file the editor (and `Scene.Save()`) writes back to.
    pub fn set_current_scene_path(&mut self, path: Option<String>) {
        *self.scene_path.borrow_mut() = path;
    }

    /// Edit the scene file held in `cell` (the script runtime's
    /// `ScriptManager::scene_path_cell`) instead of a private one, so the editor
    /// and scripts share one current scene file (#1366).
    pub fn share_scene_path(&mut self, cell: Rc<RefCell<Option<String>>>) {
        self.scene_path = cell;
    }
}
