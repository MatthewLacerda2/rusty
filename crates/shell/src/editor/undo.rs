//! Only the editor's own work records: the dashboard, then the viewport's pointer edits. The
//! watch, the frame's followers and the render are no edit.

use super::EditorFrontend;
use crate::Shell;
use rusty_app::GameWorld;
use rusty_editor::ViewportInteraction;

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

    fn record_edits(&self, game: &GameWorld) {
        game.scene().borrow_mut().begin_edits();
    }

    fn pause_edits(&self, game: &GameWorld) {
        game.scene().borrow_mut().pause_edits();
    }

    /// End the frame's recording: close the step unless a gesture holds it open.
    fn close_edits(&mut self, game: &GameWorld) {
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
    fn held(&self) -> bool {
        self.egui_ctx.input(|i| i.pointer.any_down())
            || rusty_editor::header::typing_holds_step(&self.egui_ctx)
            || self.editor_ui.gizmo_drag.is_some()
            || self.editor_ui.rect_drag.is_some()
    }

    /// Reading resets `history_stepped`: an Undo or Redo, whose shortcut egui consumed.
    fn touched(&mut self) -> bool {
        let stepped = std::mem::take(&mut self.editor_ui.history_stepped);
        stepped || self.egui_ctx.input(|i| i.events.iter().any(is_user_input))
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
