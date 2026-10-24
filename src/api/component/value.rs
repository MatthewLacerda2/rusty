//! src/api/component/value.rs — field values between Lua, JSON and the component
//! types (#1369).
//!
//! `Set`'s table is walked here rather than by mlua's serializer, so it reads the
//! way the named setters do: a whole-number float is an integer, and a list is
//! rebuilt from its largest index, so a `nil` hole (`Selectable.sprites`) keeps the
//! entries after it. [`decode`] then takes a value as a field's type, and [`same`]
//! says whether a written value is the one the field already holds.

use mlua::{Lua, LuaSerdeExt, Table};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value};

/// How deep a field value's tables may nest.
const MAX_DEPTH: u32 = 32;

/// A value as a Lua table, its lists marked with mlua's array metatable so that an
/// empty one goes back to `Set` as a list, not an empty table of fields. `nil`
/// for null: a field that holds nothing is absent, a list entry a hole.
pub fn to_lua(lua: &Lua, value: &Value) -> mlua::Result<mlua::Value> {
    let options = mlua::serde::SerializeOptions::new()
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false);
    lua.to_value_with(value, options)
}

/// A Lua value as JSON: tables with the array metatable `Get` puts on a list, or
/// with only positive integer keys, are lists (a missing index is null); any other
/// table is a table of fields.
pub fn from_lua(lua: &Lua, value: &mlua::Value) -> Result<Value, String> {
    walk(lua, value, 0)
}

fn walk(lua: &Lua, value: &mlua::Value, depth: u32) -> Result<Value, String> {
    Ok(match value {
        mlua::Value::Nil => Value::Null,
        v if v.is_null() => Value::Null,
        mlua::Value::Boolean(b) => Value::Bool(*b),
        mlua::Value::Integer(i) => Value::from(*i),
        mlua::Value::Number(n) => number(*n)?,
        mlua::Value::String(s) => Value::String(s.to_str().map_err(|e| e.to_string())?.to_owned()),
        mlua::Value::Table(t) if depth < MAX_DEPTH => table(lua, t, depth + 1)?,
        mlua::Value::Table(_) => return Err(format!("tables nest deeper than {MAX_DEPTH}")),
        other => return Err(format!("a {} is not a field value", other.type_name())),
    })
}

/// A Lua float as the named setters take it: whole numbers are integers.
fn number(n: f64) -> Result<Value, String> {
    // 2^53: every whole float below it is an exact integer.
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        return Ok(Value::from(n as i64));
    }
    Number::from_f64(n)
        .map(Value::Number)
        .ok_or_else(|| format!("{n} is not a finite number"))
}

fn table(lua: &Lua, t: &Table, depth: u32) -> Result<Value, String> {
    let marked = t
        .metatable()
        .is_some_and(|m| m.to_pointer() == lua.array_metatable().to_pointer());
    let pairs: Vec<(mlua::Value, mlua::Value)> = t
        .pairs()
        .collect::<mlua::Result<_>>()
        .map_err(|e| e.to_string())?;
    let index = |k: &mlua::Value| match k {
        mlua::Value::Integer(i) if *i >= 1 => Some(*i as usize),
        _ => None,
    };
    if marked || (!pairs.is_empty() && pairs.iter().all(|(k, _)| index(k).is_some())) {
        let len = pairs
            .iter()
            .filter_map(|(k, _)| index(k))
            .max()
            .unwrap_or(0);
        if len > 2 * pairs.len() + 16 {
            return Err(format!(
                "a list of {} entries ends at index {len}",
                pairs.len()
            ));
        }
        let mut list = vec![Value::Null; len];
        for (k, v) in &pairs {
            let i = index(k).ok_or("a list has a key that is not an index")?;
            list[i - 1] = walk(lua, v, depth)?;
        }
        return Ok(Value::Array(list));
    }
    let mut map = Map::new();
    for (k, v) in &pairs {
        let mlua::Value::String(key) = k else {
            return Err(format!(
                "a table mixes field names and a {} key",
                k.type_name()
            ));
        };
        let key = key.to_str().map_err(|e| e.to_string())?.to_owned();
        map.insert(key, walk(lua, v, depth)?);
    }
    Ok(Value::Object(map))
}

/// Decode a field's value as `T` (enum names in any case, `lua_json::decode`). An
/// empty Lua table crosses as `{}`, so when that fails it is read again with every
/// empty table a list (`LODGroup.levels[i].renderers = {}`).
pub fn decode<T: DeserializeOwned>(json: Value) -> Result<T, String> {
    let error = match crate::api::lua_json::decode(json.clone()) {
        Ok(value) => return Ok(value),
        Err(e) => e,
    };
    let mut lists = json;
    if empty_tables_as_lists(&mut lists) {
        crate::api::lua_json::decode(lists).map_err(|_| error)
    } else {
        Err(error)
    }
}

/// Replace every empty object in `value` with an empty list; whether any was.
fn empty_tables_as_lists(value: &mut Value) -> bool {
    match value {
        Value::Object(map) if map.is_empty() => {
            *value = Value::Array(Vec::new());
            true
        }
        Value::Object(map) => map
            .values_mut()
            .fold(false, |any, v| empty_tables_as_lists(v) | any),
        Value::Array(list) => list
            .iter_mut()
            .fold(false, |any, v| empty_tables_as_lists(v) | any),
        _ => false,
    }
}

/// Whether `written` is the value a field `held` already: numbers by value, an
/// absent field or list entry the same as null, and an empty table as an empty
/// list, so a table `Get` read and `Set` hands back unchanged compares equal.
pub fn same(written: &Value, held: &Value) -> bool {
    match (written, held) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => (0..a.len().max(b.len())).all(|i| {
            same(
                a.get(i).unwrap_or(&Value::Null),
                b.get(i).unwrap_or(&Value::Null),
            )
        }),
        (Value::Object(a), Value::Object(b)) => a.keys().chain(b.keys()).all(|k| {
            same(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
            )
        }),
        (Value::Object(m), Value::Array(l)) | (Value::Array(l), Value::Object(m)) => {
            m.is_empty() && l.is_empty()
        }
        _ => written == held,
    }
}

/// A bit mask as the named setters take one (`Camera.SetCullingMask`): any Lua
/// integer, kept to its low 32 bits, so `-1` is every bit.
pub struct Mask(pub u32);

impl<'de> Deserialize<'de> for Mask {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        i64::deserialize(d).map(|m| Mask(m as u32))
    }
}
