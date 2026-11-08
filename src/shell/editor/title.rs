//! src/shell/editor/title.rs — the editor window's title, Unity's place for the
//! open scene's name (#1914): `<project> - <scene>`, with a `*` while the edit scene
//! has unsaved changes (`EditorUi::is_dirty`). The window is told only when the
//! string changes. The player keeps its launch title.
//!
//! The title also keeps the dirty mark honest across Play: what Play edits is
//! thrown away on Stop, so the mark Play found is put back when it ends, and a
//! scene swapped in outside Play (a scripted `Scene.Load`) starts clean.

use winit::window::Window;

use crate::app::GameWorld;
use crate::editor::EditorUi;
use crate::scene::SceneId;

/// The window title for `scene` in `project`, marked `*` when `dirty`.
pub(super) fn text(project: &str, scene: &str, dirty: bool) -> String {
    let mark = if dirty { "*" } else { "" };
    format!("{project} - {scene}{mark}")
}

/// What the title was last set to and what it needs to keep the dirty mark right.
pub(super) struct Title {
    /// The project's folder name, as Unity titles its window.
    project: String,
    /// The title the window holds; empty until the first frame sets it.
    shown: String,
    /// The edit scene's identity, to notice a swap outside Play.
    scene: Option<SceneId>,
    /// The dirty mark when Play began; `Some` while playing.
    dirty_at_play: Option<bool>,
}

impl Title {
    pub(super) fn new() -> Self {
        let root = crate::core::project::root();
        let project = root.file_name().map(|n| n.to_string_lossy().into_owned());
        Self {
            project: project.unwrap_or_else(|| "rusty".to_string()),
            shown: String::new(),
            scene: None,
            dirty_at_play: None,
        }
    }

    /// Each frame, after the editor drew: settle the dirty mark, then retitle the
    /// window if the title changed.
    pub(super) fn sync(&mut self, window: &Window, ui: &mut EditorUi, game: &GameWorld) {
        let (playing, id) = {
            let scene = game.scene().borrow();
            (game.play_requested() || scene.is_playing(), scene.id())
        };
        self.track(&mut ui.is_dirty, playing, id);
        let title = text(&self.project, &ui.scene_name(), ui.is_dirty);
        if title != self.shown {
            window.set_title(&title);
            self.shown = title;
        }
    }

    /// Keep `dirty` to the edit scene: remember it when Play begins, put it back when
    /// Play ends, and clear it when another scene arrives outside Play.
    fn track(&mut self, dirty: &mut bool, playing: bool, id: SceneId) {
        match (playing, self.dirty_at_play) {
            (true, None) => self.dirty_at_play = Some(*dirty),
            (true, Some(_)) => return,
            (false, Some(before)) => {
                *dirty = before;
                self.dirty_at_play = None;
            }
            (false, None) if self.scene.is_some_and(|last| last != id) => *dirty = false,
            (false, None) => {}
        }
        self.scene = Some(id);
    }
}

#[cfg(test)]
#[path = "title_tests.rs"]
mod tests;
