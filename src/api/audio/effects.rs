//! src/api/audio/effects.rs — the `Audio` group-effect verbs (#1748).
//!
//! `SetGroupEffects(group, { {kind = "compressor", threshold = -18, ...}, ... })`
//! replaces a group's ordered effects; `GetGroupEffects(group)` reads them back
//! with every parameter. The tables cross as the mixer's neutral fields
//! (`audio::mixer::effects::Fields`), so the checks — an unknown kind or field, a
//! value out of range, a group that does not exist — live with the effects and
//! reach the script as Lua errors. A snapshot's `effects` entry is read here too.
//!
//! A list must be a list: a table with a non-array key (`{ kind = "limiter" }`
//! where `{ { kind = "limiter" } }` was meant) or a hole is an error naming the
//! field, never read as a shorter list. Tables nest one level only (an effect's
//! `bands`), so a table that holds itself is an error, not a stack overflow.

use mlua::{Lua, Table};

use super::super::{put, Reg};
use super::groups::err;
use crate::api::ApiCell;
use crate::audio::mixer::effects::{Fields, Value};
use crate::audio::AudioMaestro;

pub(super) fn register_effects(lua: &Lua, table: &Table, audio: ApiCell<AudioMaestro>) -> Reg {
    put(
        table,
        "SetGroupEffects",
        lua.create_function(move |lua, (name, list): (String, Table)| {
            let tables = field_list(&list, "effects")?;
            let audio = audio.get(lua)?;
            let chain = audio.borrow().mixer().parse_effects(&tables).map_err(err)?;
            audio
                .borrow_mut()
                .set_group_effects(&name, chain)
                .map_err(err)?;
            Ok(())
        }),
    )?;
    put(
        table,
        "GetGroupEffects",
        lua.create_function(move |lua, name: String| {
            let audio = &*audio.get(lua)?;
            let maestro = audio.borrow();
            let id = maestro.mixer().require(&name).map_err(err)?;
            let list = lua.create_table()?;
            for fields in maestro.mixer().effect_fields(id) {
                list.push(to_lua(lua, &fields)?)?;
            }
            Ok(list)
        }),
    )
}

/// A list of effect tables, the field `what`, as fields in order.
pub(super) fn field_list(list: &Table, what: &str) -> mlua::Result<Vec<Fields>> {
    tables(list, what, 0)
}

/// `list` (the field `what`, `depth` lists deep) as fields in order: a table with
/// keys `1..=n` and no others, each value a table.
fn tables(list: &Table, what: &str, depth: usize) -> mlua::Result<Vec<Fields>> {
    let n = list.raw_len();
    for pair in list.pairs::<mlua::Value, mlua::Value>() {
        let (key, _) = pair?;
        match key {
            mlua::Value::Integer(i) if (1..=n as i64).contains(&i) => {}
            mlua::Value::Integer(i) if i > 0 => {
                return Err(err(format!("'{what}' has a hole before entry {i}")));
            }
            other => {
                let key = other
                    .to_string()
                    .unwrap_or_else(|_| other.type_name().into());
                return Err(err(format!(
                    "'{what}' must be a list of tables ({{ {{...}}, ... }}), not a table with key '{key}'"
                )));
            }
        }
    }
    (1..=n)
        .map(|i| match list.raw_get::<mlua::Value>(i)? {
            mlua::Value::Table(t) => from_lua(&t, depth),
            mlua::Value::Nil => Err(err(format!("'{what}' has a hole at entry {i}"))),
            other => Err(err(format!(
                "'{what}' entry {i} must be a table, not a {}",
                other.type_name()
            ))),
        })
        .collect()
}

/// One table (`depth` lists deep) as fields: numbers, strings and, at the top
/// level only, a nested list of tables.
fn from_lua(t: &Table, depth: usize) -> mlua::Result<Fields> {
    let mut fields = Fields::new();
    for pair in t.pairs::<String, mlua::Value>() {
        let (key, value) = pair?;
        let value = match value {
            mlua::Value::Integer(n) => Value::Number(n as f64),
            mlua::Value::Number(n) => Value::Number(n),
            mlua::Value::String(s) => Value::Text(s.to_str()?.to_string()),
            mlua::Value::Table(list) if depth == 0 => Value::List(tables(&list, &key, 1)?),
            mlua::Value::Table(_) => {
                return Err(err(format!(
                    "effect field '{key}' cannot be a table: only an effect's own fields nest"
                )));
            }
            other => {
                let kind = other.type_name();
                return Err(err(format!("effect field '{key}' cannot be a {kind}")));
            }
        };
        fields.push((key, value));
    }
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(fields)
}

/// Fields as a table.
fn to_lua(lua: &Lua, fields: &Fields) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    for (key, value) in fields {
        match value {
            Value::Number(n) => t.set(key.as_str(), *n)?,
            Value::Text(s) if s.is_empty() => {}
            Value::Text(s) => t.set(key.as_str(), s.as_str())?,
            Value::List(entries) => {
                let list = lua.create_table()?;
                for entry in entries {
                    list.push(to_lua(lua, entry)?)?;
                }
                t.set(key.as_str(), list)?;
            }
        }
    }
    Ok(t)
}
