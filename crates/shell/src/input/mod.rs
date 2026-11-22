//! crates/shell/src/input/mod.rs — the input pump and the OS cursor policy, shared by both
//! frontends.

pub mod clipboard;
pub mod clipboard_source;
pub mod cursor;
pub mod keys;
pub mod pad_source;
pub mod pads;
pub mod view;

use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::KeyCode;

use rusty_app::GameWorld;
use rusty_core::input::keymap::Keymap;

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

fn typed_text(text: &str) -> String {
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
