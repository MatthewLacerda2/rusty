//! src/api/audio/effects.rs — the `Audio` group-effect verbs (#1748).
//!
//! `SetGroupEffects(group, { {kind = "compressor", threshold = -18, ...}, ... })`
//! replaces a group's ordered effects; `GetGroupEffects(group)` reads them back
//! with every parameter. The tables cross as the mixer's neutral fields
//! (`audio::mixer::effects::Fields`), so the checks — an unknown kind or field, a
//! value out of range, a group that does not exist — live with the effects and
//! reach the script as Lua errors. A snapshot's `effects` entry is read here too.

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
            let tables = field_list(&list)?;
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

/// A list of effect tables as fields, in order.
pub(super) fn field_list(list: &Table) -> mlua::Result<Vec<Fields>> {
    list.sequence_values::<Table>()
        .map(|t| from_lua(&t?))
        .collect()
}

/// One table as fields: numbers, strings and nested lists of tables.
fn from_lua(t: &Table) -> mlua::Result<Fields> {
    let mut fields = Fields::new();
    for pair in t.pairs::<String, mlua::Value>() {
        let (key, value) = pair?;
        let value = match value {
            mlua::Value::Integer(n) => Value::Number(n as f64),
            mlua::Value::Number(n) => Value::Number(n),
            mlua::Value::String(s) => Value::Text(s.to_str()?.to_string()),
            mlua::Value::Table(list) => Value::List(field_list(&list)?),
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
