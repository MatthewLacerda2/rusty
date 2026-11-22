use mlua::Value;

use crate::api::lua_json::recipe_from_lua;
use rusty_shadergen::ShaderRecipe;

pub fn parse_recipe(value: &Value) -> Result<ShaderRecipe, String> {
    recipe_from_lua(value)
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;
    use rusty_shadergen::PassKind;

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
                    { id = "grayscale" },
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
