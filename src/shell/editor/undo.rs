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
use crate::editor::ViewportInteraction;
use crate::shell::Shell;

impl EditorFrontend {
    /// Draw the dashboard, recording what it edits.
    pub(super) fn recorded_dashboard(
        &mut self,
        shell: &mut Shell,
        game: &mut GameWorld,
    ) -> ViewportInteraction {
        self.record_edits(game);
        let interaction = self.draw_dashboard(shell, game);
        self.pause_edits(game);
        interaction
    }

    /// Apply the viewport's pointer edits (picking, the gizmo, the rect tool),
    /// recorded, then close the frame's step.
    pub(super) fn recorded_viewport_interaction(
        &mut self,
        game: &GameWorld,
        interaction: &ViewportInteraction,
        pixels_per_point: f32,
    ) {
        self.record_edits(game);
        self.handle_viewport_interaction(game, interaction, pixels_per_point);
        self.close_edits(game);
    }

    /// Record the writes that follow as the editor's edit: a full one when someone
    /// is acting on the editor, a cheap follow-up otherwise.
    fn record_edits(&self, game: &GameWorld) {
        let mut scene = game.scene().borrow_mut();
        if self.held() || self.touched() {
            scene.begin_edits();
        } else {
            scene.begin_follow_up();
        }
    }

    /// Stop recording until the next [`Self::record_edits`].
    fn pause_edits(&self, game: &GameWorld) {
        game.scene().borrow_mut().pause_edits();
    }

    /// End the frame's recording: close the step unless a gesture holds it open.
    fn close_edits(&self, game: &GameWorld) {
        let (held, touched) = (self.held(), self.touched());
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

impl EditorFrontend {
    /// Whether a gesture holds the step open: a button down, a gizmo or rect-tool
    /// drag, a text field holding the keyboard.
    fn held(&self) -> bool {
        self.egui_ctx.input(|i| i.pointer.any_down())
            || self.egui_ctx.egui_wants_keyboard_input()
            || self.editor_ui.gizmo_drag.is_some()
            || self.editor_ui.rect_drag.is_some()
    }

    /// Whether someone acted on the editor this frame (a press, a key, text).
    fn touched(&self) -> bool {
        self.egui_ctx.input(|i| i.events.iter().any(is_user_input))
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
