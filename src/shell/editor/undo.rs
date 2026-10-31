//! src/shell/editor/undo.rs — where the editor's frame opens and closes its Undo
//! steps (#1631, `scene::undo`).
//!
//! A frame records only while the editor itself runs (the dashboard, then the
//! viewport's pointer edits); the watch, the frame's followers and the render
//! are not edits. A step closes when the frame ends with no gesture held: no
//! button down, no gizmo or rect-tool drag, no text field being typed into.
//! A frame nobody touched folds what it changed into the last step instead;
//! camera navigation (the wheel, a held fly key) is no touch.

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

    /// Record the writes that follow as the editor's edit.
    fn record_edits(&self, game: &GameWorld) {
        game.scene().borrow_mut().begin_edits();
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
    /// drag, a text field being typed into (not the console, not a widget that
    /// only kept focus after its gesture: `editor::header::typing_holds_step`).
    fn held(&self) -> bool {
        self.egui_ctx.input(|i| i.pointer.any_down())
            || crate::editor::header::typing_holds_step(&self.egui_ctx)
            || self.editor_ui.gizmo_drag.is_some()
            || self.editor_ui.rect_drag.is_some()
    }

    /// Whether someone acted on the editor this frame (a press, a key, text).
    fn touched(&self) -> bool {
        self.egui_ctx.input(|i| i.events.iter().any(is_user_input))
    }
}

/// Whether `event` is someone acting on the editor, not the pointer only
/// passing or the camera navigating (the wheel zooms, a held key repeats).
fn is_user_input(event: &egui::Event) -> bool {
    !matches!(
        event,
        egui::Event::PointerMoved(_)
            | egui::Event::MouseMoved(_)
            | egui::Event::PointerGone
            | egui::Event::WindowFocused(_)
            | egui::Event::MouseWheel { .. }
            | egui::Event::Zoom(_)
            | egui::Event::Key { repeat: true, .. }
    )
}

#[cfg(test)]
mod tests {
    use super::is_user_input;

    #[test]
    fn camera_navigation_is_no_touch_and_a_press_is() {
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 1.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        };
        let key = |repeat| egui::Event::Key {
            key: egui::Key::W,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers: egui::Modifiers::NONE,
        };
        assert!(!is_user_input(&wheel));
        assert!(!is_user_input(&egui::Event::Zoom(1.1)));
        assert!(!is_user_input(&key(true)), "a held fly key repeats");
        assert!(is_user_input(&key(false)));
    }
}
