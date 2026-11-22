use mlua::{Lua, LuaSerdeExt, Value};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub use rusty_core::naming::variant_case::decode;

pub fn recipe_from_lua<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    decode(recipe_json(value)?)
}

/// A test hook: the root's `tests/` read a recipe as a script's JSON string is read.
pub fn recipe_from_json_str<T: DeserializeOwned>(json: &str) -> Result<T, String> {
    decode(serde_json::from_str(json).map_err(|e| e.to_string())?)
}

pub fn recipe_json(value: &Value) -> Result<serde_json::Value, String> {
    match value {
        Value::Table(_) => lua_to_json(value),
        Value::String(s) => {
            serde_json::from_str(&s.to_str().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        }
        other => Err(format!(
            "recipe must be a table or a JSON string, got {}",
            other.type_name()
        )),
    }
}

pub fn lua_to_json(value: &Value) -> Result<serde_json::Value, String> {
    let mut json = serde_json::to_value(value.to_serializable().detect_mixed_tables(true))
        .map_err(|e| e.to_string())?;
    empty_arrays_to_objects(&mut json);
    Ok(json)
}

/// mlua reads an empty table as `[]`, rusty answers `{}`; only an empty table yields an empty
/// array, so rewriting every `[]` is exact.
fn empty_arrays_to_objects(json: &mut serde_json::Value) {
    match json {
        serde_json::Value::Array(items) if items.is_empty() => {
            *json = serde_json::Value::Object(serde_json::Map::new());
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(empty_arrays_to_objects),
        serde_json::Value::Object(map) => map.values_mut().for_each(empty_arrays_to_objects),
        _ => {}
    }
}

pub fn to_lua<T: Serialize + ?Sized>(lua: &Lua, value: &T) -> mlua::Result<Value> {
    let options = mlua::serde::SerializeOptions::new()
        .set_array_metatable(false)
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false);
    lua.to_value_with(value, options)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod enum_case_tests;

#[cfg(test)]
mod recipe_tests;
