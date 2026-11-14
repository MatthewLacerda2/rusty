//! crates/script/src/api/storage.rs — `Storage` namespace.
//!
//! The Lua/REPL/harness surface over `core::storage::Storage` (issue #86). Scripts
//! read/write namespaced values that survive across runs; a value may be a scalar
//! or a whole structured table. Only the in-memory map is touched here — disk I/O
//! happens at boundaries (Stop / quit), never from a script during a tick, which
//! keeps the sim a pure function of its inputs.

use mlua::{Lua, Table, Value as LuaValue};

use super::lua_json::to_lua;
use super::{put, Reg};
use crate::api::ApiCell;
use rusty_core::storage::Storage;

/// Register the `Storage` namespace onto `lua`.
pub fn register(lua: &Lua, storage: ApiCell<Storage>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_scalars(lua, &table, storage)?;
    register_namespaces(lua, &table, storage)?;

    lua.globals()
        .set("Storage", table)
        .map_err(|e| e.to_string())
}

/// Per-key accessors: `Set` / `Get` / `Has` / `Delete`.
fn register_scalars(lua: &mlua::Lua, table: &Table, storage: ApiCell<Storage>) -> Reg {
    put(
        table,
        "Set",
        lua.create_function(move |lua, (ns, key, value): (String, String, LuaValue)| {
            let json = lua_to_json(&value).map_err(mlua::Error::RuntimeError)?;
            storage.get(lua)?.borrow_mut().set(&ns, &key, json);
            Ok(())
        }),
    )?;

    put(
        table,
        "Get",
        lua.create_function(move |lua, (ns, key): (String, String)| {
            match storage.get(lua)?.borrow().get(&ns, &key) {
                Some(json) => to_lua(lua, &json),
                None => Ok(LuaValue::Nil),
            }
        }),
    )?;

    put(
        table,
        "Has",
        lua.create_function(move |lua, (ns, key): (String, String)| {
            Ok(storage.get(lua)?.borrow().has(&ns, &key))
        }),
    )?;

    put(
        table,
        "Delete",
        lua.create_function(move |lua, (ns, key): (String, String)| {
            Ok(storage.get(lua)?.borrow_mut().delete(&ns, &key))
        }),
    )
}

/// Whole-namespace blob accessors — store/read a structured table at once.
fn register_namespaces(lua: &mlua::Lua, table: &Table, storage: ApiCell<Storage>) -> Reg {
    put(
        table,
        "GetTable",
        lua.create_function(move |lua, ns: String| {
            match storage.get(lua)?.borrow().get_namespace(&ns) {
                Some(json) => to_lua(lua, &json),
                None => Ok(LuaValue::Nil),
            }
        }),
    )?;

    put(
        table,
        "SetTable",
        lua.create_function(move |lua, (ns, value): (String, Table)| {
            let json = lua_to_json(&LuaValue::Table(value)).map_err(mlua::Error::RuntimeError)?;
            storage.get(lua)?.borrow_mut().set_namespace(&ns, json);
            Ok(())
        }),
    )
}

/// A script value as JSON for storage, through the shared serde bridge
/// ([`lua_json`](super::lua_json)). Functions/userdata are rejected.
fn lua_to_json(value: &LuaValue) -> Result<serde_json::Value, String> {
    super::lua_json::lua_to_json(value)
        .map_err(|e| format!("Storage cannot persist this value: {e}"))
}
