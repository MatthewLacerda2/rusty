//! src/api/storage.rs — `Storage` namespace.
//!
//! The Lua/REPL/harness surface over `core::storage::Storage` (issue #86). Scripts
//! read/write namespaced values that survive across runs; a value may be a scalar
//! or a whole structured table. Only the in-memory map is touched here — disk I/O
//! happens at boundaries (Stop / quit), never from a script during a tick, which
//! keeps the sim a pure function of its inputs.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Table, Value as LuaValue};
use serde_json::Value as JsonValue;

use super::{put, Reg};
use crate::core::storage::Storage;

/// Register the `Storage` namespace onto `lua`.
#[allow(clippy::too_many_lines)] // legacy; burn down in #124
pub fn register(lua: &Lua, storage: &Rc<RefCell<Storage>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(storage);
    put(
        &table,
        "Set",
        lua.create_function(move |_, (ns, key, value): (String, String, LuaValue)| {
            let json = lua_to_json(&value).map_err(mlua::Error::RuntimeError)?;
            s.borrow_mut().set(&ns, &key, json);
            Ok(())
        }),
    )?;

    let s = Rc::clone(storage);
    put(
        &table,
        "Get",
        lua.create_function(move |lua, (ns, key): (String, String)| {
            match s.borrow().get(&ns, &key) {
                Some(json) => json_to_lua(lua, &json),
                None => Ok(LuaValue::Nil),
            }
        }),
    )?;

    let s = Rc::clone(storage);
    put(
        &table,
        "Has",
        lua.create_function(move |_, (ns, key): (String, String)| Ok(s.borrow().has(&ns, &key))),
    )?;

    let s = Rc::clone(storage);
    put(
        &table,
        "Delete",
        lua.create_function(move |_, (ns, key): (String, String)| {
            Ok(s.borrow_mut().delete(&ns, &key))
        }),
    )?;

    // Whole-namespace blob accessors — store/read a structured table at once.
    let s = Rc::clone(storage);
    put(
        &table,
        "GetTable",
        lua.create_function(move |lua, ns: String| match s.borrow().get_namespace(&ns) {
            Some(json) => json_to_lua(lua, &json),
            None => Ok(LuaValue::Nil),
        }),
    )?;

    let s = Rc::clone(storage);
    put(
        &table,
        "SetTable",
        lua.create_function(move |_, (ns, value): (String, Table)| {
            let json = lua_to_json(&LuaValue::Table(value)).map_err(mlua::Error::RuntimeError)?;
            s.borrow_mut().set_namespace(&ns, json);
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Storage", table)
        .map_err(|e| e.to_string())
}

/// Convert an mlua value into JSON for storage. Functions/userdata are rejected.
fn lua_to_json(value: &LuaValue) -> Result<JsonValue, String> {
    match value {
        LuaValue::Nil => Ok(JsonValue::Null),
        LuaValue::Boolean(b) => Ok(JsonValue::Bool(*b)),
        LuaValue::Integer(i) => Ok((*i).into()),
        LuaValue::Number(n) => Ok(serde_json::Number::from_f64(*n)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null)),
        LuaValue::String(s) => Ok(JsonValue::String(
            s.to_str().map_err(|e| e.to_string())?.to_owned(),
        )),
        LuaValue::Table(t) => table_to_json(t),
        other => Err(format!(
            "Storage cannot persist a Lua {}",
            other.type_name()
        )),
    }
}

/// Convert a Lua table to JSON: a clean 1..n integer-keyed sequence becomes an
/// array, anything else an object (numeric keys stringified).
fn table_to_json(t: &Table) -> Result<JsonValue, String> {
    let len = t.raw_len();
    let total = t.clone().pairs::<LuaValue, LuaValue>().count();

    if len == total && len > 0 {
        let mut arr = Vec::with_capacity(len);
        for i in 1..=len {
            let v: LuaValue = t.raw_get(i).map_err(|e| e.to_string())?;
            arr.push(lua_to_json(&v)?);
        }
        return Ok(JsonValue::Array(arr));
    }

    let mut map = serde_json::Map::new();
    for pair in t.clone().pairs::<LuaValue, LuaValue>() {
        let (k, v) = pair.map_err(|e| e.to_string())?;
        let key = match k {
            LuaValue::String(s) => s.to_str().map_err(|e| e.to_string())?.to_owned(),
            LuaValue::Integer(i) => i.to_string(),
            LuaValue::Number(n) => n.to_string(),
            other => {
                return Err(format!(
                    "Storage table keys must be strings or numbers, got {}",
                    other.type_name()
                ))
            }
        };
        map.insert(key, lua_to_json(&v)?);
    }
    Ok(JsonValue::Object(map))
}

/// Convert stored JSON back into an mlua value.
fn json_to_lua<'lua>(lua: &'lua Lua, value: &JsonValue) -> mlua::Result<LuaValue<'lua>> {
    Ok(match value {
        JsonValue::Null => LuaValue::Nil,
        JsonValue::Bool(b) => LuaValue::Boolean(*b),
        JsonValue::Number(n) => match n.as_i64() {
            Some(i) => LuaValue::Integer(i),
            None => LuaValue::Number(n.as_f64().unwrap_or(0.0)),
        },
        JsonValue::String(s) => LuaValue::String(lua.create_string(s)?),
        JsonValue::Array(items) => {
            let t = lua.create_table()?;
            for (i, item) in items.iter().enumerate() {
                t.raw_set(i + 1, json_to_lua(lua, item)?)?;
            }
            LuaValue::Table(t)
        }
        JsonValue::Object(map) => {
            let t = lua.create_table()?;
            for (k, v) in map {
                t.raw_set(k.as_str(), json_to_lua(lua, v)?)?;
            }
            LuaValue::Table(t)
        }
    })
}
