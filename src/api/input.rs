//! src/api/input.rs — `Input` namespace.
//!
//! The readable half (key state + per-tick edges, the mouse, typed text) and the
//! cursor request are registered first; the WRITABLE half (`Press`/`Release`,
//! `MoveMouse`, `AddMouseDelta`, `Scroll`, `TypeText`) extends the same table.
//! Writable input is what lets a script, bot or the harness play as the user
//! (Unity-ish `Input` injection); injected input bypasses the keymap. Gamepads (#471)
//! add named axes, pad connection, dead zones and rumble to both halves. The
//! clipboard (#612) is one pair in the readable half: a game copies with the same
//! `SetClipboard` a bot uses to stage a paste.

use std::cell::RefCell;

use mlua::Lua;

use super::{global_table, put, Reg};
use crate::core::gamepad::{DeadZones, Rumble};
use crate::core::input::InputState;

/// Register the readable half of `Input` and the cursor request onto `lua`,
/// creating the `Input` global table.
pub fn register_readable<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    input: &'scope RefCell<InputState>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "IsKeyDown",
        scope.create_function(|_, key: String| Ok(input.borrow().is_key_down(&key))),
    )?;
    put(
        &table,
        "GetKeyDown",
        scope.create_function(|_, key: String| Ok(input.borrow().get_key_down(&key))),
    )?;
    put(
        &table,
        "GetKeyUp",
        scope.create_function(|_, key: String| Ok(input.borrow().get_key_up(&key))),
    )?;
    put(
        &table,
        "GetMousePosition",
        scope.create_function(|_, ()| Ok(input.borrow().mouse_position())),
    )?;
    put(
        &table,
        "GetMouseDelta",
        scope.create_function(|_, ()| Ok(input.borrow().mouse_delta())),
    )?;
    put(
        &table,
        "GetScrollDelta",
        scope.create_function(|_, ()| Ok(input.borrow().scroll_delta())),
    )?;
    put(
        &table,
        "GetTextInput",
        scope.create_function(|_, ()| Ok(input.borrow().text_input().to_string())),
    )?;
    register_cursor(scope, &table, input)?;
    register_clipboard(scope, &table, input)?;
    register_pad(scope, &table, input)?;

    lua.globals().set("Input", table).map_err(|e| e.to_string())
}

/// The cursor request: the sim records it, the platform applies it to the OS cursor.
fn register_cursor<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    input: &'scope RefCell<InputState>,
) -> Reg {
    put(
        table,
        "SetCursorLocked",
        scope.create_function(|_, locked: bool| {
            input.borrow_mut().set_cursor_locked(locked);
            Ok(())
        }),
    )?;
    put(
        table,
        "SetCursorVisible",
        scope.create_function(|_, visible: bool| {
            input.borrow_mut().set_cursor_visible(visible);
            Ok(())
        }),
    )?;
    put(
        table,
        "IsCursorLocked",
        scope.create_function(|_, ()| Ok(input.borrow().cursor().locked)),
    )?;
    put(
        table,
        "IsCursorVisible",
        scope.create_function(|_, ()| Ok(input.borrow().cursor().visible)),
    )
}

/// The clipboard: the text the platform captured (or the game last set), and the
/// write the platform copies to the OS clipboard after the tick.
fn register_clipboard<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    input: &'scope RefCell<InputState>,
) -> Reg {
    put(
        table,
        "GetClipboard",
        scope.create_function(|_, ()| Ok(input.borrow().clipboard.text().to_string())),
    )?;
    put(
        table,
        "SetClipboard",
        scope.create_function(|_, text: String| {
            input.borrow_mut().clipboard.set(&text);
            Ok(())
        }),
    )
}

/// The gamepad reads, and the one pad *output* (rumble): axes, connection, dead
/// zones, and rumble requests the platform plays (a record headless).
fn register_pad<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    input: &'scope RefCell<InputState>,
) -> Reg {
    put(
        table,
        "GetAxis",
        scope.create_function(|_, axis: String| Ok(input.borrow().axis(&axis))),
    )?;
    put(
        table,
        "IsPadConnected",
        scope.create_function(|_, pad: usize| Ok(input.borrow().pads.is_connected(pad))),
    )?;
    put(
        table,
        "GetDeadZone",
        scope.create_function(|_, ()| {
            let zones = input.borrow().pads.dead_zones;
            Ok((zones.stick, zones.trigger))
        }),
    )?;
    put(
        table,
        "SetDeadZone",
        scope.create_function(|_, (stick, trigger): (f32, f32)| {
            input.borrow_mut().pads.dead_zones = DeadZones::clamped(stick, trigger);
            Ok(())
        }),
    )?;
    put(
        table,
        "SetRumble",
        scope.create_function(|_, (pad, low, high, seconds): (usize, f32, f32, f32)| {
            let rumble = Rumble::new(low, high, seconds);
            input.borrow_mut().pads.request_rumble(pad, rumble);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetRumble",
        scope.create_function(|_, pad: usize| {
            let rumble = input.borrow().pads.last_rumble(pad);
            Ok((rumble.low, rumble.high, rumble.seconds))
        }),
    )
}

/// Add the writable half of `Input`: injection that drives the shared input state
/// so a script can play as the user. Extends the existing `Input` table.
pub fn register_writable<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    input: &'scope RefCell<InputState>,
) -> Reg {
    let table = global_table(lua, "Input")?;

    put(
        &table,
        "Press",
        scope.create_function(|_, key: String| {
            input.borrow_mut().press(&key);
            Ok(())
        }),
    )?;
    put(
        &table,
        "Release",
        scope.create_function(|_, key: String| {
            input.borrow_mut().release(&key);
            Ok(())
        }),
    )?;
    put(
        &table,
        "MoveMouse",
        scope.create_function(|_, (x, y): (f64, f64)| {
            input.borrow_mut().move_mouse(x, y);
            Ok(())
        }),
    )?;
    put(
        &table,
        "AddMouseDelta",
        scope.create_function(|_, (dx, dy): (f64, f64)| {
            input.borrow_mut().add_mouse_delta(dx, dy);
            Ok(())
        }),
    )?;
    put(
        &table,
        "Scroll",
        scope.create_function(|_, dy: f64| {
            input.borrow_mut().scroll(dy);
            Ok(())
        }),
    )?;
    put(
        &table,
        "TypeText",
        scope.create_function(|_, text: String| {
            input.borrow_mut().type_text(&text);
            Ok(())
        }),
    )?;
    register_pad_injection(scope, &table, input)
}

/// Pad injection: axis values and pad (dis)connection, as a bot or the harness drives
/// them. Logical, like `Press`: no dead zone, no keymap.
fn register_pad_injection<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    input: &'scope RefCell<InputState>,
) -> Reg {
    put(
        table,
        "SetAxis",
        scope.create_function(|_, (axis, value): (String, f32)| {
            input.borrow_mut().set_axis(&axis, value);
            Ok(())
        }),
    )?;
    put(
        table,
        "SetPadConnected",
        scope.create_function(|_, (pad, connected): (usize, bool)| {
            input.borrow_mut().pads.set_connected(pad, connected);
            Ok(())
        }),
    )
}
