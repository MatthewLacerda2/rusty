//! src/api/texture/from_lua.rs — marshal a Lua recipe table into a `TextureRecipe`.
//!
//! mlua's `serde` feature is not enabled in this project, so the Lua → recipe bridge
//! is written by hand against the table API. The Lua shape mirrors the serde document
//! one-to-one (so a recipe authored in Lua and one loaded from `.json` describe the
//! same DAG):
//!
//! ```lua
//! {
//!   resolution = 1024, seed = 42, output = "n1",
//!   nodes = {
//!     { id = "n0", op = "checker", tiles = 4,
//!       color_a = {0,0,0,1}, color_b = {1,1,1,1} },
//!     { id = "n1", op = "invert", inputs = {"n0"} },
//!   },
//! }
//! ```
//!
//! Each node table carries `id`, an `op` tag string, the op's params as fields, and an
//! optional `inputs` array — exactly the internally-tagged serde form. Marshaling goes
//! Lua table → `serde_json::Value` → `TextureRecipe`, so the op-param decoding lives
//! ONCE in `recipe`'s serde derive rather than being duplicated here.

use mlua::{Table, Value};

use crate::procgen::TextureRecipe;

/// Parse a Lua recipe `table` into a [`TextureRecipe`]. Errors carry a message the
/// REPL/script surfaces verbatim.
pub fn recipe_from_table(table: &Table) -> Result<TextureRecipe, String> {
    let json = lua_table_to_json(table)?;
    TextureRecipe::from_json(&serde_json::to_string(&json).map_err(|e| e.to_string())?)
}

/// Convert a Lua value to a `serde_json::Value`. Tables become JSON arrays when they
/// are a 1..n integer sequence, else JSON objects — the heuristic that lets `inputs`
/// / `color_a` ride as arrays and node/op fields ride as objects.
fn lua_value_to_json(value: &Value) -> Result<serde_json::Value, String> {
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
        Value::Table(t) => lua_table_to_json(t),
        other => Err(format!("unsupported Lua value in recipe: {other:?}")),
    }
}

/// Convert a Lua table to JSON: an array if it is a dense `1..=len` sequence, else an
/// object keyed by its string keys.
fn lua_table_to_json(table: &Table) -> Result<serde_json::Value, String> {
    let len = table.raw_len();
    if len > 0 && is_pure_sequence(table, len) {
        let mut arr = Vec::with_capacity(len);
        for i in 1..=len {
            let v: Value = table.raw_get(i).map_err(|e| e.to_string())?;
            arr.push(lua_value_to_json(&v)?);
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
        map.insert(key, lua_value_to_json(&v)?);
    }
    Ok(serde_json::Value::Object(map))
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
    fn parses_a_two_node_recipe_table() {
        let lua = Lua::new();
        let table: Table = lua
            .load(
                r#"
                return {
                  resolution = 32, seed = 5, output = "n1",
                  nodes = {
                    { id = "n0", op = "checker", tiles = 4,
                      color_a = {0,0,0,1}, color_b = {1,1,1,1} },
                    { id = "n1", op = "invert", inputs = {"n0"} },
                  },
                }
            "#,
            )
            .eval()
            .unwrap();
        let recipe = recipe_from_table(&table).expect("recipe parses");
        assert_eq!(recipe.resolution, 32);
        assert_eq!(recipe.seed, 5);
        assert_eq!(recipe.nodes.len(), 2);
        assert_eq!(recipe.output.as_deref(), Some("n1"));
        assert_eq!(recipe.nodes[1].inputs, vec!["n0".to_string()]);
    }

    #[test]
    fn reports_unknown_op_tag() {
        let lua = Lua::new();
        let table: Table = lua
            .load(r#"return { resolution = 8, nodes = { { id = "x", op = "bogus" } } }"#)
            .eval()
            .unwrap();
        assert!(recipe_from_table(&table).is_err());
    }
}
