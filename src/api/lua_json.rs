//! src/api/lua_json.rs — marshal a Lua table into a `serde_json::Value`.
//!
//! mlua's `serde` feature is not enabled in this project, so any recipe-table API
//! (`Texture.Bake`, `Material.DefineAsset`) bridges Lua → struct by hand: convert the
//! Lua table to a `serde_json::Value`, then `serde_json::from_value` into the target
//! struct so the field decoding lives ONCE in that struct's serde derive. This module
//! is the shared converter both surfaces call, so the table→JSON heuristic is written
//! once rather than per namespace.
//!
//! The heuristic: a Lua table becomes a JSON **array** when it is a dense `1..=len`
//! integer sequence (so `inputs` / `color_a` / `base_color` ride as arrays), else a
//! JSON **object** keyed by its string keys (node/op/material fields). That matches how
//! a recipe authored in Lua and one loaded from `.json` describe the same document.

use mlua::{Table, Value};

/// Convert a Lua `table` into a `serde_json::Value`. Errors carry a message the
/// REPL/script surfaces verbatim.
pub fn table_to_json(table: &Table) -> Result<serde_json::Value, String> {
    let len = table.raw_len();
    if len > 0 && is_pure_sequence(table, len) {
        let mut arr = Vec::with_capacity(len);
        for i in 1..=len {
            let v: Value = table.raw_get(i).map_err(|e| e.to_string())?;
            arr.push(value_to_json(&v)?);
        }
        return Ok(serde_json::Value::Array(arr));
    }

    let mut map = serde_json::Map::new();
    for pair in table.clone().pairs::<Value, Value>() {
        let (k, v) = pair.map_err(|e| e.to_string())?;
        let key = match k {
            Value::String(s) => s.to_str().map_err(|e| e.to_string())?.to_string(),
            Value::Integer(i) => i.to_string(),
            other => return Err(format!("unsupported recipe key: {other:?}")),
        };
        map.insert(key, value_to_json(&v)?);
    }
    Ok(serde_json::Value::Object(map))
}

/// Convert one Lua value to a `serde_json::Value`. Tables recurse through
/// [`table_to_json`]; non-finite numbers are an error (they can't survive JSON).
fn value_to_json(value: &Value) -> Result<serde_json::Value, String> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Integer(i) => Ok(serde_json::Value::from(*i)),
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "non-finite number in recipe".to_string()),
        Value::String(s) => Ok(serde_json::Value::String(
            s.to_str().map_err(|e| e.to_string())?.to_string(),
        )),
        Value::Table(t) => table_to_json(t),
        other => Err(format!("unsupported Lua value in recipe: {other:?}")),
    }
}

/// A table is a "pure sequence" when keys `1..=len` all exist and there are no extra
/// string keys — i.e. a plain array, not a mixed table.
fn is_pure_sequence(table: &Table, len: usize) -> bool {
    for i in 1..=len {
        let v: Value = match table.raw_get(i) {
            Ok(v) => v,
            Err(_) => return false,
        };
        if matches!(v, Value::Nil) {
            return false;
        }
    }
    let len = len as i64;
    // No non-integer keys beyond the sequence.
    table
        .clone()
        .pairs::<Value, Value>()
        .filter_map(Result::ok)
        .all(|(k, _)| matches!(k, Value::Integer(i) if (1..=len).contains(&i)))
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn object_table_round_trips_keys() {
        let lua = Lua::new();
        let t: Table = lua
            .load(r#"return { metallic = 0.5, render_mode = "Cutout" }"#)
            .eval()
            .unwrap();
        let json = table_to_json(&t).unwrap();
        assert_eq!(json["metallic"], serde_json::json!(0.5));
        assert_eq!(json["render_mode"], serde_json::json!("Cutout"));
    }

    #[test]
    fn dense_sequence_becomes_an_array() {
        let lua = Lua::new();
        let t: Table = lua.load(r#"return {0.1, 0.2, 0.3}"#).eval().unwrap();
        let json = table_to_json(&t).unwrap();
        assert_eq!(json, serde_json::json!([0.1, 0.2, 0.3]));
    }

    #[test]
    fn nested_arrays_inside_objects() {
        let lua = Lua::new();
        let t: Table = lua
            .load(r#"return { base_color = {1.0, 0.5, 0.25} }"#)
            .eval()
            .unwrap();
        let json = table_to_json(&t).unwrap();
        assert_eq!(json["base_color"], serde_json::json!([1.0, 0.5, 0.25]));
    }
}
