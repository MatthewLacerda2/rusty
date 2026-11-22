//! crates/shell/src/editor/cursor_release.rs — Esc in Play frees the cursor.

use winit::keyboard::KeyCode;

use crate::input::CursorPolicy;
use rusty_core::input::CursorState;
use rusty_editor::{ViewportInteraction, ViewportTab};

/// Whether the editor has freed the cursor over the game's lock request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorRelease {
    released: bool,
}

impl CursorRelease {
    /// Esc pressed while the game has input frees the cursor (the key itself has
    /// already gone to the game).
    pub fn on_key(&mut self, key: KeyCode, pressed: bool, game_has_input: bool) {
        if key == KeyCode::Escape && pressed && game_has_input {
            self.released = true;
        }
    }

    /// A click inside the Game view hands the cursor back to the game's request.
    pub fn on_pointer(&mut self, interaction: &ViewportInteraction) {
        let inside = interaction.tab == ViewportTab::Game && interaction.hover_local.is_some();
        if interaction.pointer_pressed && inside {
            self.released = false;
        }
    }

    /// Entering or leaving Play starts from the game's own request.
    pub fn reset(&mut self) {
        self.released = false;
    }

    /// The OS cursor: the game's request while it has input and the editor has not
    /// freed it, otherwise a free, visible cursor.
    pub fn effective(self, requested: CursorState, game_has_input: bool) -> CursorState {
        CursorPolicy::effective(requested, game_has_input && !self.released)
    }
}

#[cfg(test)]
#[path = "cursor_release_tests.rs"]
mod cursor_release_tests;
