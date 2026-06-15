//! src/api/debug.rs — `Debug` namespace (DEV-ONLY).
//!
//! `Debug.Log/Warn/Error`. Registered only in dev builds — stripped from the
//! shipped game, like Unity `Debug.*` under `[Conditional]`. The whole module is
//! gated behind the `dev` feature in `api::mod`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::scripting::ConsoleLogs;

/// Register the `Debug` namespace onto `lua` (dev builds only).
pub fn register(lua: &Lua, console: &Rc<RefCell<ConsoleLogs>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let c = Rc::clone(console);
    put(
        &table,
        "Log",
        lua.create_function(move |_, msg: String| {
            c.borrow_mut().info(msg);
            Ok(())
        }),
    )?;

    let c = Rc::clone(console);
    put(
        &table,
        "Warn",
        lua.create_function(move |_, msg: String| {
            c.borrow_mut().warn(msg);
            Ok(())
        }),
    )?;

    let c = Rc::clone(console);
    put(
        &table,
        "Error",
        lua.create_function(move |_, msg: String| {
            c.borrow_mut().error(msg);
            Ok(())
        }),
    )?;

    lua.globals().set("Debug", table).map_err(|e| e.to_string())
}
