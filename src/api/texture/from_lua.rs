//! src/api/texture/from_lua.rs — marshal a Lua recipe table into a `TextureRecipe`.
//!
//! mlua's `serde` feature is not enabled in this project, so the Lua → recipe bridge
//! goes Lua table → `serde_json::Value` (via the shared [`crate::api::lua_json`]
//! converter) → `TextureRecipe`, so the op-param decoding lives ONCE in `recipe`'s
//! serde derive. The Lua shape mirrors the serde document one-to-one (so a recipe
//! authored in Lua and one loaded from `.json` describe the same DAG):
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
//! optional `inputs` array — exactly the internally-tagged serde form.

use mlua::Table;

use crate::api::lua_json::table_to_json;
use crate::procgen::TextureRecipe;

/// Parse a Lua recipe `table` into a [`TextureRecipe`]. Errors carry a message the
/// REPL/script surfaces verbatim.
pub fn recipe_from_table(table: &Table) -> Result<TextureRecipe, String> {
    let json = table_to_json(table)?;
    TextureRecipe::from_json(&serde_json::to_string(&json).map_err(|e| e.to_string())?)
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
