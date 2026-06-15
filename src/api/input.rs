//! src/api/input.rs — `Input` namespace.
//!
//! Read (`IsKeyDown`) and the WRITABLE half (`Press`/`Release`). Writable input
//! is what lets a script play as the user (Unity-ish `Input` injection). The
//! readable half is registered first; the writable half extends the same table.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{global_table, put, Reg};
use crate::core::input::InputState;

/// Register the readable half of `Input` (`IsKeyDown`) onto `lua`, creating the
/// `Input` global table.
pub fn register_readable(lua: &Lua, input: &Rc<RefCell<InputState>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let i = Rc::clone(input);
    put(
        &table,
        "IsKeyDown",
        lua.create_function(move |_, key_name: String| Ok(i.borrow().is_key_down(&key_name))),
    )?;

    lua.globals().set("Input", table).map_err(|e| e.to_string())
}

/// Add the writable half of `Input`: `Press`/`Release` drive the shared input
/// state so a script can play as the user. Extends the existing `Input` table.
pub fn register_writable(lua: &Lua, input: &Rc<RefCell<InputState>>) -> Reg {
    let table = global_table(lua, "Input")?;

    let i = Rc::clone(input);
    put(
        &table,
        "Press",
        lua.create_function(move |_, key: String| {
            i.borrow_mut().press(&key);
            Ok(())
        }),
    )?;

    let i = Rc::clone(input);
    put(
        &table,
        "Release",
        lua.create_function(move |_, key: String| {
            i.borrow_mut().release(&key);
            Ok(())
        }),
    )?;

    Ok(())
}
