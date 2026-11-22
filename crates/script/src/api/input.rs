use mlua::Lua;

use super::{global_table, put, Reg};
use crate::api::ApiCell;
use rusty_core::input::gamepad::{DeadZones, Rumble};
use rusty_core::input::InputState;

pub fn register(lua: &Lua, input: ApiCell<InputState>) -> Reg {
    register_readable(lua, input)?;
    register_writable(lua, input)
}

/// A test hook: `tests/input_api.rs` and `tests/gamepad_api.rs` register each half.
pub fn register_readable(lua: &Lua, input: ApiCell<InputState>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "IsKeyDown",
        lua.create_function(move |lua, key: String| Ok(input.get(lua)?.borrow().is_key_down(&key))),
    )?;
    put(
        &table,
        "GetKeyDown",
        lua.create_function(move |lua, key: String| {
            Ok(input.get(lua)?.borrow().get_key_down(&key))
        }),
    )?;
    put(
        &table,
        "GetKeyUp",
        lua.create_function(move |lua, key: String| Ok(input.get(lua)?.borrow().get_key_up(&key))),
    )?;
    put(
        &table,
        "GetMousePosition",
        lua.create_function(move |lua, ()| Ok(input.get(lua)?.borrow().mouse_position())),
    )?;
    put(
        &table,
        "GetMouseDelta",
        lua.create_function(move |lua, ()| Ok(input.get(lua)?.borrow().mouse_delta())),
    )?;
    put(
        &table,
        "GetScrollDelta",
        lua.create_function(move |lua, ()| Ok(input.get(lua)?.borrow().scroll_delta())),
    )?;
    put(
        &table,
        "GetTextInput",
        lua.create_function(move |lua, ()| Ok(input.get(lua)?.borrow().text_input().to_string())),
    )?;
    register_cursor(lua, &table, input)?;
    register_clipboard(lua, &table, input)?;
    register_pad(lua, &table, input)?;

    lua.globals().set("Input", table).map_err(|e| e.to_string())
}

fn register_cursor(lua: &mlua::Lua, table: &mlua::Table, input: ApiCell<InputState>) -> Reg {
    put(
        table,
        "SetCursorLocked",
        lua.create_function(move |lua, locked: bool| {
            input.get(lua)?.borrow_mut().set_cursor_locked(locked);
            Ok(())
        }),
    )?;
    put(
        table,
        "SetCursorVisible",
        lua.create_function(move |lua, visible: bool| {
            input.get(lua)?.borrow_mut().set_cursor_visible(visible);
            Ok(())
        }),
    )?;
    put(
        table,
        "IsCursorLocked",
        lua.create_function(move |lua, ()| Ok(input.get(lua)?.borrow().cursor().locked)),
    )?;
    put(
        table,
        "IsCursorVisible",
        lua.create_function(move |lua, ()| Ok(input.get(lua)?.borrow().cursor().visible)),
    )
}

fn register_clipboard(lua: &mlua::Lua, table: &mlua::Table, input: ApiCell<InputState>) -> Reg {
    put(
        table,
        "GetClipboard",
        lua.create_function(move |lua, ()| {
            Ok(input.get(lua)?.borrow().clipboard.text().to_string())
        }),
    )?;
    put(
        table,
        "SetClipboard",
        lua.create_function(move |lua, text: String| {
            input.get(lua)?.borrow_mut().clipboard.set(&text);
            Ok(())
        }),
    )
}

fn register_pad(lua: &mlua::Lua, table: &mlua::Table, input: ApiCell<InputState>) -> Reg {
    put(
        table,
        "GetAxis",
        lua.create_function(move |lua, axis: String| Ok(input.get(lua)?.borrow().axis(&axis))),
    )?;
    put(
        table,
        "IsPadConnected",
        lua.create_function(move |lua, pad: usize| {
            Ok(input.get(lua)?.borrow().pads.is_connected(pad))
        }),
    )?;
    put(
        table,
        "GetDeadZone",
        lua.create_function(move |lua, ()| {
            let zones = input.get(lua)?.borrow().pads.dead_zones;
            Ok((zones.stick, zones.trigger))
        }),
    )?;
    put(
        table,
        "SetDeadZone",
        lua.create_function(move |lua, (stick, trigger): (f32, f32)| {
            input.get(lua)?.borrow_mut().pads.dead_zones = DeadZones::clamped(stick, trigger);
            Ok(())
        }),
    )?;
    register_rumble(lua, table, input)
}

fn register_rumble(lua: &mlua::Lua, table: &mlua::Table, input: ApiCell<InputState>) -> Reg {
    put(
        table,
        "SetRumble",
        lua.create_function(
            move |lua, (pad, low, high, seconds): (usize, f32, f32, f32)| {
                let rumble = Rumble::new(low, high, seconds);
                input
                    .get(lua)?
                    .borrow_mut()
                    .pads
                    .request_rumble(pad, rumble);
                Ok(())
            },
        ),
    )?;
    put(
        table,
        "GetRumble",
        lua.create_function(move |lua, pad: usize| {
            let rumble = input.get(lua)?.borrow().pads.last_rumble(pad);
            Ok((rumble.low, rumble.high, rumble.seconds))
        }),
    )
}

/// A test hook: `tests/input_api.rs` and `tests/gamepad_api.rs` register each half.
pub fn register_writable(lua: &Lua, input: ApiCell<InputState>) -> Reg {
    let table = global_table(lua, "Input")?;

    put(
        &table,
        "Press",
        lua.create_function(move |lua, key: String| {
            input.get(lua)?.borrow_mut().press(&key);
            Ok(())
        }),
    )?;
    put(
        &table,
        "Release",
        lua.create_function(move |lua, key: String| {
            input.get(lua)?.borrow_mut().release(&key);
            Ok(())
        }),
    )?;
    put(
        &table,
        "MoveMouse",
        lua.create_function(move |lua, (x, y): (f64, f64)| {
            input.get(lua)?.borrow_mut().move_mouse(x, y);
            Ok(())
        }),
    )?;
    put(
        &table,
        "AddMouseDelta",
        lua.create_function(move |lua, (dx, dy): (f64, f64)| {
            input.get(lua)?.borrow_mut().add_mouse_delta(dx, dy);
            Ok(())
        }),
    )?;
    put(
        &table,
        "Scroll",
        lua.create_function(move |lua, dy: f64| {
            input.get(lua)?.borrow_mut().scroll(dy);
            Ok(())
        }),
    )?;
    put(
        &table,
        "TypeText",
        lua.create_function(move |lua, text: String| {
            input.get(lua)?.borrow_mut().type_text(&text);
            Ok(())
        }),
    )?;
    register_pad_injection(lua, &table, input)
}

fn register_pad_injection(lua: &mlua::Lua, table: &mlua::Table, input: ApiCell<InputState>) -> Reg {
    put(
        table,
        "SetAxis",
        lua.create_function(move |lua, (axis, value): (String, f32)| {
            input.get(lua)?.borrow_mut().set_axis(&axis, value);
            Ok(())
        }),
    )?;
    put(
        table,
        "SetPadConnected",
        lua.create_function(move |lua, (pad, connected): (usize, bool)| {
            input
                .get(lua)?
                .borrow_mut()
                .pads
                .set_connected(pad, connected);
            Ok(())
        }),
    )
}
