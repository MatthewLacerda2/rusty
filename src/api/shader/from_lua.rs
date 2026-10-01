//! src/api/shader/from_lua.rs — marshal a Lua shader-recipe table into a
//! [`ShaderRecipe`] (#272).
//!
//! mlua's `serde` feature is not enabled, so the Lua → recipe bridge goes Lua
//! table → `serde_json::Value` (via the shared [`crate::api::lua_json`] converter)
//! → `ShaderRecipe`, so the recipe decoding lives ONCE in the serde derive. The
//! Lua shape mirrors the serde document one-to-one (so a recipe authored in Lua
//! and one loaded from `.json` describe the same module):
//!
//! ```lua
//! {
//!   pass = "postfx",            -- "surface" | "postfx"
//!   name = "warm_grade",        -- baked file becomes <name>.wgsl
//!   blocks = {
//!     { id = "tint", params = { color = {1.0, 0.8, 0.6} } },
//!     { id = "vignette", params = { strength = 0.6 } },
//!   },
//! }
//! ```
//!
//! Each block table carries an `id` and an optional `params` map; a param is a
//! scalar or a small float array.

use mlua::Value;

use crate::api::lua_json::recipe_from_lua;
use crate::shadergen::ShaderRecipe;

/// Parse a recipe argument — a Lua table or its JSON string (#410) — into a
/// [`ShaderRecipe`]. Errors carry a message the REPL/script surfaces verbatim.
pub fn parse_recipe(value: &Value) -> Result<ShaderRecipe, String> {
    recipe_from_lua(value)
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;
    use crate::shadergen::PassKind;

    #[test]
    fn parses_a_postfx_recipe_table() {
        let lua = Lua::new();
        let table: Value = lua
            .load(
                r#"
                return {
                  pass = "postfx", name = "warm",
                  blocks = {
                    { id = "tint", params = { color = {1.0, 0.8, 0.6} } },
                    { id = "vignette" },
                  },
                }
            "#,
            )
            .eval()
            .unwrap();
        let recipe = parse_recipe(&table).expect("recipe parses");
        assert_eq!(recipe.pass, PassKind::Postfx);
        assert_eq!(recipe.name, "warm");
        assert_eq!(recipe.blocks.len(), 2);
        assert_eq!(recipe.blocks[0].id, "tint");
    }

    #[test]
    fn reports_unknown_pass_kind() {
        let lua = Lua::new();
        let table: Value = lua
            .load(r#"return { pass = "bogus", name = "x", blocks = {} }"#)
            .eval()
            .unwrap();
        assert!(parse_recipe(&table).is_err());
    }
}
