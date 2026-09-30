//! src/shell/input/mod.rs — the input pump and the OS cursor policy, shared by both
//! frontends.
//!
//! The window speaks physical keys; the sim speaks logical ones. [`write_key`] names
//! the physical key ([`keys`]), remaps it through the [`Keymap`], and writes the
//! logical state into `InputState` — the one place windowed input enters the sim.
//! Mouse buttons go the same way as keys (`MOUSE0`…), so they are rebindable too.
//! The pointer is mapped into game-view pixels ([`view`]); raw motion, the wheel and
//! typed text accumulate until the next sim tick publishes them. Bots and the harness
//! inject logical input directly and bypass all of this.
//!
//! [`CursorPolicy`] applies the game's cursor request to the OS cursor ([`cursor`]).

pub mod cursor;
pub mod keys;
pub mod view;

use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::KeyCode;

use crate::app::GameWorld;
use crate::core::keymap::Keymap;

pub use cursor::CursorPolicy;
pub use keys::{mouse_button_name, physical_key_name};
pub use view::GameViewRect;

/// Name a physical key, remap it to its logical key, and write the logical state.
pub fn write_key(key: KeyCode, pressed: bool, game: &GameWorld, keymap: &Keymap) {
    if let Some(physical) = physical_key_name(key) {
        let logical = keymap.resolve(physical);
        game.input().borrow_mut().set_key_state(&logical, pressed);
    }
}

/// A mouse button is a key: name it, remap it, write it.
pub fn write_mouse_button(
    button: MouseButton,
    state: ElementState,
    game: &GameWorld,
    keymap: &Keymap,
) {
    let logical = keymap.resolve(&mouse_button_name(button));
    let pressed = state == ElementState::Pressed;
    game.input().borrow_mut().set_key_state(&logical, pressed);
}

/// Move the pointer, mapping the window position into the game view.
pub fn write_cursor_moved(window: (f64, f64), view: &GameViewRect, game: &GameWorld) {
    let (x, y) = view.to_view(window);
    game.input().borrow_mut().move_mouse(x, y);
}

/// Accumulate raw mouse motion for the next tick's `GetMouseDelta`.
pub fn write_mouse_motion(delta: (f64, f64), game: &GameWorld) {
    game.input().borrow_mut().add_mouse_delta(delta.0, delta.1);
}

/// Accumulate wheel motion (in lines) for the next tick's `GetScrollDelta`.
pub fn write_wheel(delta: MouseScrollDelta, game: &GameWorld) {
    game.input().borrow_mut().scroll(view::scroll_lines(delta));
}

/// Append typed text for the next tick's `GetTextInput`.
pub fn write_text(text: &str, game: &GameWorld) {
    let typed = typed_text(text);
    if !typed.is_empty() {
        game.input().borrow_mut().type_text(&typed);
    }
}

/// What a key's OS text contributes to `GetTextInput`, Unity's `inputString` rules:
/// printable characters, backspace as `"\b"`, Enter as `"\n"`, Tab as `"\t"`; any
/// other control character (Esc, Delete, …) is dropped.
pub fn typed_text(text: &str) -> String {
    text.chars()
        .filter_map(|c| match c {
            '\r' => Some('\n'),
            '\u{8}' | '\n' | '\t' => Some(c),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::typed_text;

    #[test]
    fn typed_text_keeps_editing_controls_and_drops_the_rest() {
        assert_eq!(typed_text("aB1 "), "aB1 ");
        assert_eq!(typed_text("\r"), "\n");
        assert_eq!(typed_text("\u{8}\t"), "\u{8}\t");
        assert_eq!(typed_text("\u{1b}\u{7f}"), "");
        assert_eq!(typed_text("é"), "é");
    }
}
