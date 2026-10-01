//! src/shell/editor/game_focus.rs — editor input routing (#416): the game gets input
//! only while playing **and** the Game view has focus.
//!
//! Entering Play switches to the Game tab and focuses it (Unity's default). A click
//! inside the Game view focuses it; a click anywhere else (Inspector, Hierarchy, the
//! Scene tab) takes focus away and frees the cursor — except while the game holds the
//! cursor locked, when the pointer is captured and a "click elsewhere" is really a
//! click at the captured position (Esc frees the cursor first, the way out, #576).

use super::EditorFrontend;
use crate::app::{GameWorld, PlayTransition};
use crate::editor::{ViewportInteraction, ViewportTab};
use crate::shell::input::GameViewRect;
use crate::shell::Frontend;

/// Whether the Game view has focus after this frame's pointer activity.
pub fn next_focus(
    was_focused: bool,
    interaction: &ViewportInteraction,
    cursor_captured: bool,
) -> bool {
    if interaction.tab != ViewportTab::Game {
        return false;
    }
    if interaction.pointer_pressed && !(was_focused && cursor_captured) {
        return interaction.hover_local.is_some();
    }
    was_focused
}

/// The Game view's rect in physical window pixels, rendered at `render` pixels.
pub fn game_view_rect(
    interaction: &ViewportInteraction,
    ppp: f32,
    render: (u32, u32),
) -> GameViewRect {
    let ppp = f64::from(ppp);
    GameViewRect {
        origin: (
            f64::from(interaction.origin.x) * ppp,
            f64::from(interaction.origin.y) * ppp,
        ),
        size: (
            f64::from(interaction.size.x) * ppp,
            f64::from(interaction.size.y) * ppp,
        ),
        render,
    }
}

impl EditorFrontend {
    /// Play focuses the Game view; Stop drops focus. Either one ends an Esc release.
    pub(super) fn focus_on_transition(&mut self, transition: PlayTransition) {
        if transition != PlayTransition::None {
            self.cursor_release.reset();
        }
        match transition {
            PlayTransition::Entered => {
                self.editor_ui.viewport_tab = ViewportTab::Game;
                self.game_focused = true;
            }
            PlayTransition::Exited => self.game_focused = false,
            PlayTransition::None => {}
        }
    }

    /// Update Game-view focus from this frame's pointer activity, releasing held keys
    /// when focus leaves so none stays stuck down in the game.
    pub(super) fn update_game_focus(
        &mut self,
        game: &GameWorld,
        interaction: &ViewportInteraction,
    ) {
        let captured = self.cursor(game).locked;
        self.cursor_release.on_pointer(interaction);
        let focused = next_focus(self.game_focused, interaction, captured);
        if self.game_focused && !focused {
            game.input().borrow_mut().release_all();
        }
        self.game_focused = focused;
    }
}

#[cfg(test)]
#[path = "game_focus_tests.rs"]
mod game_focus_tests;
