//! src/api/lua_json.rs — rusty's settings for mlua's serde bridge (`LuaSerdeExt`).
//!
//! The conversion itself is mlua's (#755); this module only fixes the options every
//! caller shares, so a recipe, a `Storage` value and a UI look all cross the Lua ↔
//! serde boundary the same way:
//!
//! - **Lua → serde**: a table is an array when its keys are all positive integers
//!   (a nil hole is a JSON `null`), else an object whose integer keys become strings;
//!   an empty table is an object. Functions, threads and userdata are an error.
//! - **serde → Lua**: `null` and `None` are plain `nil` (not mlua's `null`
//!   sentinel) and arrays carry no metatable, so a script sees ordinary tables.
//!
//! Every recipe verb (`Texture.Bake`, `Shader.Bake`, `Sound.Bake`,
//! `Material.DefineAsset`, …) takes its recipe through [`recipe_from_lua`], which
//! accepts **either** form — a Lua table or the recipe's JSON string (#410) — so
//! there is one verb per operation, not a table verb plus a `*Json` twin. Both go
//! through `serde_json::Value`, so a bad key reads the same whichever form carried it.

use mlua::{Lua, LuaSerdeExt, Value};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Decode a recipe argument into `T`: a Lua **table** or a **JSON string** (the
/// on-disk form). Both reach `T`'s serde derive through the same `from_value` call.
pub fn recipe_from_lua<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_json::from_value(recipe_json(value)?).map_err(|e| e.to_string())
}

/// The recipe argument as a `serde_json::Value`, for a caller that must inspect
/// the document before decoding it (`Material.DefineAsset` lifts its validated
/// fields out first). Anything but a table or a string is an error naming both.
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

/// Any Lua value as a `serde_json::Value`, with the table rules above.
pub fn lua_to_json(value: &Value) -> Result<serde_json::Value, String> {
    let mut json = serde_json::to_value(value.to_serializable().detect_mixed_tables(true))
        .map_err(|e| e.to_string())?;
    empty_arrays_to_objects(&mut json);
    Ok(json)
}

/// mlua's mixed-table detection reads an empty table as `[]`; rusty has always
/// answered `{}`. A Lua table yields an empty array only when it is empty, so
/// rewriting every `[]` restores that answer exactly.
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

/// Any serializable value as a plain Lua value (`nil` for null, no array metatable).
pub fn to_lua<T: Serialize + ?Sized>(lua: &Lua, value: &T) -> mlua::Result<Value> {
    let options = mlua::serde::SerializeOptions::new()
        .set_array_metatable(false)
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false);
    lua.to_value_with(value, options)
}

#[cfg(test)]
#[path = "lua_json_tests.rs"]
mod tests;
