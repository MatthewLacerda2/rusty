//! src/shell/input/cursor.rs — applying the game's cursor request to the OS cursor.
//!
//! The sim records what the game wants ([`CursorState`]: Play defaults to locked +
//! hidden, `Input.SetCursorLocked` / `SetCursorVisible` override it); the shell
//! applies it here, in both frontends, whenever the effective state changes. When the
//! game does not have input (edit mode, or the editor's Game view unfocused) the
//! cursor is always free, so a script can never trap the editor's pointer.

use crate::core::input::CursorState;

/// What the platform last did with the OS cursor.
#[derive(Clone, Copy, Debug, Default)]
pub struct CursorPolicy {
    applied: Option<CursorState>,
}

impl CursorPolicy {
    /// The cursor the window should have: the game's request while it has input,
    /// otherwise a free, visible cursor.
    pub fn effective(requested: CursorState, game_has_input: bool) -> CursorState {
        if game_has_input {
            requested
        } else {
            CursorState::FREE
        }
    }

    /// Record `want`; returns it when it differs from what was last applied (so the
    /// caller touches the window only on a change).
    pub fn update(&mut self, want: CursorState) -> Option<CursorState> {
        (self.applied != Some(want)).then(|| {
            self.applied = Some(want);
            want
        })
    }

    /// Bring the window's cursor in line with `want`, if it changed.
    pub fn sync(&mut self, want: CursorState, window: &winit::window::Window) {
        if let Some(cursor) = self.update(want) {
            apply(cursor, window);
        }
    }
}

/// Apply a cursor state to the window. A lock prefers `Locked` (the pointer stays
/// put, raw motion keeps flowing — macOS/Wayland) and falls back to `Confined`
/// (X11/Windows, which lack `Locked`).
fn apply(cursor: CursorState, window: &winit::window::Window) {
    use winit::window::CursorGrabMode;
    if cursor.locked {
        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
            window.set_cursor_grab(CursorGrabMode::Confined).ok();
        }
    } else {
        window.set_cursor_grab(CursorGrabMode::None).ok();
    }
    window.set_cursor_visible(cursor.visible);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_applies_only_while_the_game_has_input() {
        let locked = CursorState::PLAY;
        assert_eq!(CursorPolicy::effective(locked, true), locked);
        assert_eq!(CursorPolicy::effective(locked, false), CursorState::FREE);
    }

    #[test]
    fn a_state_is_applied_once_per_change() {
        let mut policy = CursorPolicy::default();
        assert_eq!(policy.update(CursorState::FREE), Some(CursorState::FREE));
        assert_eq!(policy.update(CursorState::FREE), None);
        assert_eq!(policy.update(CursorState::PLAY), Some(CursorState::PLAY));
    }
}
