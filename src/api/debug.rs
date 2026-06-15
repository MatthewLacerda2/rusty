//! src/api/debug.rs — `Debug` namespace (DEV-ONLY).
//!
//! `Debug.Log/Warn/Error`. Registered only in dev builds — stripped from the
//! shipped game, like Unity `Debug.*` under `[Conditional]`. The whole module is
//! gated behind the `dev` feature in `api::mod`. Scoped callbacks over a borrowed
//! `&RefCell<&mut ConsoleLogs>` (#57).

use std::cell::RefCell;

use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::scripting::ConsoleLogs;

/// Register the `Debug` namespace onto `lua` (dev builds only).
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    console: &'scope RefCell<&'scope mut ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Log",
        scope.create_function(move |_, msg: String| {
            console.borrow_mut().info(msg);
            Ok(())
        }),
    )?;

    put(
        &table,
        "Warn",
        scope.create_function(move |_, msg: String| {
            console.borrow_mut().warn(msg);
            Ok(())
        }),
    )?;

    put(
        &table,
        "Error",
        scope.create_function(move |_, msg: String| {
            console.borrow_mut().error(msg);
            Ok(())
        }),
    )?;

    lua.globals().set("Debug", table).map_err(|e| e.to_string())
}
