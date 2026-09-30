//! src/api/input.rs — `Input` namespace.
//!
//! The readable half (key state + per-tick edges, the mouse, typed text) and the
//! cursor request are registered first; the WRITABLE half (`Press`/`Release`,
//! `MoveMouse`, `AddMouseDelta`, `Scroll`, `TypeText`) extends the same table.
//! Writable input is what lets a script, bot or the harness play as the user
//! (Unity-ish `Input` injection); injected input bypasses the keymap.

use std::cell::RefCell;

use mlua::Lua;

use super::{global_table, put, Reg};
use crate::core::input::InputState;

/// Register the readable half of `Input` and the cursor request onto `lua`,
/// creating the `Input` global table.
pub fn register_readable<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
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

    lua.globals().set("Input", table).map_err(|e| e.to_string())
}

/// The cursor request: the sim records it, the platform applies it to the OS cursor.
fn register_cursor<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table<'lua>,
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

/// Add the writable half of `Input`: injection that drives the shared input state
/// so a script can play as the user. Extends the existing `Input` table.
pub fn register_writable<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
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
    )
}
