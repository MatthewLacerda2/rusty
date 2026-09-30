//! src/shell/input.rs — the input pump and the OS cursor policy, shared by both
//! frontends.
//!
//! The window speaks physical keys; the sim speaks logical ones. [`write_key`] names
//! the physical key, remaps it through the [`Keymap`], and writes the logical state
//! into `InputState` — the one place windowed input enters the sim. Bots and the
//! harness inject logical keys directly and bypass this.
//!
//! [`CursorPolicy`] is what the platform does with the OS cursor. Today it follows
//! play state (locked + hidden in Play, free in edit mode); #416 makes it
//! script-controllable (`Input.SetCursorLocked` / `SetCursorVisible` record a request
//! in the sim, the shell applies it here), which is why both frontends go through it.

use winit::keyboard::KeyCode;

use crate::app::{GameWorld, PlayTransition};
use crate::core::keymap::Keymap;

/// The stable physical-key name (uppercase) the engine speaks for a winit `KeyCode`,
/// or `None` for keys the sim has no logical name for yet (#416 widens this table).
pub fn physical_key_name(key: KeyCode) -> Option<&'static str> {
    Some(match key {
        KeyCode::KeyW => "W",
        KeyCode::KeyA => "A",
        KeyCode::KeyS => "S",
        KeyCode::KeyD => "D",
        KeyCode::ArrowUp => "UP",
        KeyCode::ArrowDown => "DOWN",
        KeyCode::ArrowLeft => "LEFT",
        KeyCode::ArrowRight => "RIGHT",
        // The shoot button is just the SPACE key; the player controller script
        // edge-detects it. No engine-side "shoot" field — gameplay reads the key
        // like any other.
        KeyCode::Space => "SPACE",
        _ => return None,
    })
}

/// Name a physical key, remap it to its logical key, and write the logical state.
pub fn write_key(key: KeyCode, pressed: bool, game: &GameWorld, keymap: &Keymap) {
    if let Some(physical) = physical_key_name(key) {
        let logical = keymap.resolve(physical);
        game.input().borrow_mut().set_key_state(&logical, pressed);
    }
}

/// What the platform does with the OS cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPolicy {
    /// Confine the cursor to the window.
    pub locked: bool,
    pub visible: bool,
}

impl CursorPolicy {
    /// The default in Play: locked and hidden, for mouse-look.
    pub const PLAY: Self = Self {
        locked: true,
        visible: false,
    };
    /// Outside Play: a free, visible cursor.
    pub const FREE: Self = Self {
        locked: false,
        visible: true,
    };

    /// The policy a play transition switches to, or `None` when nothing changed.
    pub fn for_transition(transition: PlayTransition) -> Option<Self> {
        match transition {
            PlayTransition::Entered => Some(Self::PLAY),
            PlayTransition::Exited => Some(Self::FREE),
            PlayTransition::None => None,
        }
    }

    /// Apply this policy to the window's cursor.
    pub fn apply(self, window: &winit::window::Window) {
        let grab = if self.locked {
            winit::window::CursorGrabMode::Confined
        } else {
            winit::window::CursorGrabMode::None
        };
        window.set_cursor_grab(grab).ok();
        window.set_cursor_visible(self.visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_locks_and_hides_stop_frees() {
        assert_eq!(
            CursorPolicy::for_transition(PlayTransition::Entered),
            Some(CursorPolicy::PLAY)
        );
        assert_eq!(
            CursorPolicy::for_transition(PlayTransition::Exited),
            Some(CursorPolicy::FREE)
        );
        assert_eq!(CursorPolicy::for_transition(PlayTransition::None), None);
    }

    #[test]
    fn movement_keys_have_names_and_others_do_not() {
        assert_eq!(physical_key_name(KeyCode::KeyW), Some("W"));
        assert_eq!(physical_key_name(KeyCode::Space), Some("SPACE"));
        assert_eq!(physical_key_name(KeyCode::F1), None);
    }
}
