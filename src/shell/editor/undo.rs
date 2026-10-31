//! src/shell/editor/undo.rs — where the editor's frame opens and closes its Undo
//! steps (#1631, `scene::undo`).
//!
//! A frame records only while the editor itself runs (the dashboard, then the
//! viewport's pointer edits); the watch, the frame's followers and the render
//! are not edits. A step closes when the frame ends with no gesture held: no
//! button down, no gizmo or rect-tool drag, no text field holding the keyboard.
//! A frame nobody touched folds what it changed into the last step instead.

use super::EditorFrontend;
use crate::app::GameWorld;

impl EditorFrontend {
    /// Record the writes that follow as the editor's edit.
    pub(super) fn record_edits(&self, game: &GameWorld) {
        game.scene().borrow_mut().begin_edits();
    }

    /// Stop recording until the next [`Self::record_edits`].
    pub(super) fn pause_edits(&self, game: &GameWorld) {
        game.scene().borrow_mut().pause_edits();
    }

    /// End the frame's recording: close the step unless a gesture holds it open.
    pub(super) fn close_edits(&self, game: &GameWorld) {
        let held = self.egui_ctx.input(|i| i.pointer.any_down())
            || self.egui_ctx.egui_wants_keyboard_input()
            || self.editor_ui.gizmo_drag.is_some()
            || self.editor_ui.rect_drag.is_some();
        let touched = self.egui_ctx.input(|i| i.events.iter().any(is_user_input));
        let mut scene = game.scene().borrow_mut();
        scene.pause_edits();
        match (held, touched) {
            (true, _) => {}
            (false, true) => {
                scene.commit_edits(None);
            }
            (false, false) => {
                scene.commit_follow_up();
            }
        }
    }
}

/// Whether `event` is someone acting on the editor, not the pointer only passing.
fn is_user_input(event: &egui::Event) -> bool {
    !matches!(
        event,
        egui::Event::PointerMoved(_)
            | egui::Event::MouseMoved(_)
            | egui::Event::PointerGone
            | egui::Event::WindowFocused(_)
    )
}
