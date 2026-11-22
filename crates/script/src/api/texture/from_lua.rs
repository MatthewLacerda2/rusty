use mlua::Value;

use crate::api::lua_json::recipe_from_lua;
use rusty_procgen::TextureRecipe;

pub fn parse_recipe(value: &Value) -> Result<TextureRecipe, String> {
    recipe_from_lua(value)
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn parses_a_two_node_recipe_table() {
        let lua = Lua::new();
        let table: Value = lua
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
        let recipe = parse_recipe(&table).expect("recipe parses");
        assert_eq!(recipe.resolution, 32);
        assert_eq!(recipe.seed, 5);
        assert_eq!(recipe.nodes.len(), 2);
        assert_eq!(recipe.output.as_deref(), Some("n1"));
        assert_eq!(recipe.nodes[1].inputs, vec!["n0".to_string()]);
    }

    #[test]
    fn a_node_missing_an_input_is_refused_naming_it() {
        let lua = Lua::new();
        let table: Value = lua
            .load(
                r#"return { resolution = 8, nodes = {
                     { id = "w", op = "white_noise" },
                     { id = "warped", op = "warp", strength = 1, inputs = {"w"} } } }"#,
            )
            .eval()
            .unwrap();
        let err = parse_recipe(&table).unwrap_err();
        assert!(
            err.contains("\"warped\"") && err.contains("1 given"),
            "{err}"
        );
    }

    #[test]
    fn reports_unknown_op_tag() {
        let lua = Lua::new();
        let table: Value = lua
            .load(r#"return { resolution = 8, nodes = { { id = "x", op = "bogus" } } }"#)
            .eval()
            .unwrap();
        assert!(parse_recipe(&table).is_err());
    }
}
