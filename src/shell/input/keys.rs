//! src/shell/input/keys.rs — the one physical-key name table (#416).
//!
//! Every winit key the engine forwards, and the stable uppercase name the sim speaks
//! for it. Names follow Unity's `KeyCode` where one exists (`LEFTSHIFT`, `EQUALS`,
//! `KEYPAD0`, …), except letters and digits, which are bare (`"W"`, `"1"`). The
//! same list is documented in `docs/api/Input.md` (Key names). Mouse
//! buttons are named in [`mouse_button_name`]; gamepad buttons in
//! [`core::gamepad`](crate::core::gamepad) (#471).

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Every forwarded key and its engine name.
pub const KEY_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"),
    (KeyCode::F12, "F12"),
    (KeyCode::F13, "F13"),
    (KeyCode::F14, "F14"),
    (KeyCode::F15, "F15"),
    (KeyCode::F16, "F16"),
    (KeyCode::F17, "F17"),
    (KeyCode::F18, "F18"),
    (KeyCode::F19, "F19"),
    (KeyCode::F20, "F20"),
    (KeyCode::F21, "F21"),
    (KeyCode::F22, "F22"),
    (KeyCode::F23, "F23"),
    (KeyCode::F24, "F24"),
    (KeyCode::ArrowUp, "UP"),
    (KeyCode::ArrowDown, "DOWN"),
    (KeyCode::ArrowLeft, "LEFT"),
    (KeyCode::ArrowRight, "RIGHT"),
    (KeyCode::Space, "SPACE"),
    (KeyCode::Escape, "ESCAPE"),
    (KeyCode::Tab, "TAB"),
    (KeyCode::Enter, "ENTER"),
    (KeyCode::Backspace, "BACKSPACE"),
    (KeyCode::Insert, "INSERT"),
    (KeyCode::Delete, "DELETE"),
    (KeyCode::Home, "HOME"),
    (KeyCode::End, "END"),
    (KeyCode::PageUp, "PAGEUP"),
    (KeyCode::PageDown, "PAGEDOWN"),
    (KeyCode::ShiftLeft, "LEFTSHIFT"),
    (KeyCode::ShiftRight, "RIGHTSHIFT"),
    (KeyCode::ControlLeft, "LEFTCONTROL"),
    (KeyCode::ControlRight, "RIGHTCONTROL"),
    (KeyCode::AltLeft, "LEFTALT"),
    (KeyCode::AltRight, "RIGHTALT"),
    (KeyCode::SuperLeft, "LEFTSUPER"),
    (KeyCode::SuperRight, "RIGHTSUPER"),
    (KeyCode::CapsLock, "CAPSLOCK"),
    (KeyCode::NumLock, "NUMLOCK"),
    (KeyCode::ScrollLock, "SCROLLLOCK"),
    (KeyCode::PrintScreen, "PRINT"),
    (KeyCode::Pause, "PAUSE"),
    (KeyCode::ContextMenu, "MENU"),
    (KeyCode::Minus, "MINUS"),
    (KeyCode::Equal, "EQUALS"),
    (KeyCode::BracketLeft, "LEFTBRACKET"),
    (KeyCode::BracketRight, "RIGHTBRACKET"),
    (KeyCode::Backslash, "BACKSLASH"),
    (KeyCode::Semicolon, "SEMICOLON"),
    (KeyCode::Quote, "QUOTE"),
    (KeyCode::Backquote, "BACKQUOTE"),
    (KeyCode::Comma, "COMMA"),
    (KeyCode::Period, "PERIOD"),
    (KeyCode::Slash, "SLASH"),
    (KeyCode::Numpad0, "KEYPAD0"),
    (KeyCode::Numpad1, "KEYPAD1"),
    (KeyCode::Numpad2, "KEYPAD2"),
    (KeyCode::Numpad3, "KEYPAD3"),
    (KeyCode::Numpad4, "KEYPAD4"),
    (KeyCode::Numpad5, "KEYPAD5"),
    (KeyCode::Numpad6, "KEYPAD6"),
    (KeyCode::Numpad7, "KEYPAD7"),
    (KeyCode::Numpad8, "KEYPAD8"),
    (KeyCode::Numpad9, "KEYPAD9"),
    (KeyCode::NumpadAdd, "KEYPADPLUS"),
    (KeyCode::NumpadSubtract, "KEYPADMINUS"),
    (KeyCode::NumpadMultiply, "KEYPADMULTIPLY"),
    (KeyCode::NumpadDivide, "KEYPADDIVIDE"),
    (KeyCode::NumpadDecimal, "KEYPADPERIOD"),
    (KeyCode::NumpadEnter, "KEYPADENTER"),
];

/// The stable physical-key name (uppercase) the engine speaks for a winit `KeyCode`,
/// or `None` for keys outside the table (media keys, IME/language keys, …).
pub fn physical_key_name(key: KeyCode) -> Option<&'static str> {
    KEY_NAMES
        .iter()
        .find(|(code, _)| *code == key)
        .map(|(_, name)| *name)
}

/// The key name for a mouse button: `MOUSE0` left, `MOUSE1` right, `MOUSE2` middle,
/// `MOUSE3` back, `MOUSE4` forward, `MOUSE<n>` for any other button winit reports.
pub fn mouse_button_name(button: MouseButton) -> String {
    let index = match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(n) => n,
    };
    format!("MOUSE{index}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn names_are_unique_uppercase_and_cover_the_keyboard() {
        let names: HashSet<&str> = KEY_NAMES.iter().map(|(_, n)| *n).collect();
        assert_eq!(names.len(), KEY_NAMES.len(), "no two keys share a name");
        assert!(names.iter().all(|n| *n == n.to_uppercase()));
        for key in [
            KeyCode::KeyW,
            KeyCode::Digit1,
            KeyCode::F12,
            KeyCode::Escape,
        ] {
            assert!(physical_key_name(key).is_some(), "{key:?} is named");
        }
        assert_eq!(physical_key_name(KeyCode::ShiftLeft), Some("LEFTSHIFT"));
        assert_eq!(physical_key_name(KeyCode::MediaPlayPause), None);
    }

    #[test]
    fn mouse_buttons_are_keys() {
        assert_eq!(mouse_button_name(MouseButton::Left), "MOUSE0");
        assert_eq!(mouse_button_name(MouseButton::Right), "MOUSE1");
        assert_eq!(mouse_button_name(MouseButton::Middle), "MOUSE2");
        assert_eq!(mouse_button_name(MouseButton::Other(7)), "MOUSE7");
    }
}
