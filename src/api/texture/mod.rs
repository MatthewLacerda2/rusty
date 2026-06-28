//! src/api/texture/mod.rs — the `Texture` namespace (#270).
//!
//! Agent-facing surface for procedural texture authoring: compose a recipe of ops and
//! bake it to a tiling `.png` a material map slot consumes. The recipe is authored as
//! a Lua table whose shape mirrors the serde document exactly (see
//! [`from_lua`](crate::api::texture::from_lua)), so the same DAG describes a Lua-built
//! recipe and one loaded from `.json` — one surface, three callers.
//!
//! Verbs:
//! - `Texture.Bake(recipe, path, slot)` — evaluate a recipe **table** and write the
//!   PNG, encoded for the named `slot` (base_color / emissive = sRGB; normal /
//!   roughness / metallic / data = linear; `metallic_roughness` packs metallic→B,
//!   roughness→G). Returns the written path.
//! - `Texture.BakeJson(json, path, slot)` — same, from a recipe JSON string (the
//!   on-disk form), so a saved recipe re-bakes without a round-trip through Lua.
//! - `Texture.ToJson(recipe)` — serialize a recipe table to its canonical JSON for
//!   saving / diffing.
//!
//! `Texture` borrows no engine state — it reads a recipe and writes a file — so it
//! registers as a plain static namespace, like `Assets`.

mod from_lua;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::procgen::{bake_recipe, Slot, TextureRecipe};
use from_lua::recipe_from_table;

/// Register the `Texture` namespace onto `lua`.
pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Bake",
        lua.create_function(|_, (recipe, path, slot): (Table, String, String)| {
            let recipe = recipe_from_table(&recipe).map_err(mlua::Error::RuntimeError)?;
            bake(&recipe, &path, &slot)
        }),
    )?;

    put(
        &table,
        "BakeJson",
        lua.create_function(|_, (json, path, slot): (String, String, String)| {
            let recipe = TextureRecipe::from_json(&json).map_err(mlua::Error::RuntimeError)?;
            bake(&recipe, &path, &slot)
        }),
    )?;

    put(
        &table,
        "ToJson",
        lua.create_function(|_, recipe: Table| {
            let recipe = recipe_from_table(&recipe).map_err(mlua::Error::RuntimeError)?;
            recipe.to_json().map_err(mlua::Error::RuntimeError)
        }),
    )?;

    lua.globals()
        .set("Texture", table)
        .map_err(|e| e.to_string())
}

/// Evaluate + bake a parsed recipe for the named slot, surfacing any error to Lua.
fn bake(recipe: &TextureRecipe, path: &str, slot: &str) -> mlua::Result<String> {
    bake_recipe(recipe, Slot::parse(slot), path).map_err(mlua::Error::RuntimeError)
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn bake_writes_a_loadable_png() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let path = std::env::temp_dir().join("rusty_texture_api_bake.png");
        let p = path.to_str().unwrap();
        // Pass the path as a Lua global *value*, not interpolated into the Lua source:
        // a Windows path (`C:\Users\…`) embedded in a Lua string literal trips Lua's
        // escape parser (`\U` is an invalid escape). As a global it's a plain string,
        // safe on every platform.
        lua.globals().set("OUT", p).unwrap();
        let script = r#"
            return Texture.Bake({
              resolution = 16, seed = 1,
              nodes = { { id = "c", op = "checker", tiles = 2,
                color_a = {0,0,0,1}, color_b = {1,1,1,1} } }
            }, OUT, "base_color")
        "#;
        let returned: String = lua.load(script).eval().unwrap();
        assert_eq!(returned, p);
        // The bytes must decode as a real PNG of the requested size.
        let img = image::open(p).expect("baked PNG decodes");
        assert_eq!(image::GenericImageView::dimensions(&img), (16, 16));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn to_json_round_trips_back_to_a_recipe() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let json: String = lua
            .load(
                r#"return Texture.ToJson({
                    resolution = 8, seed = 2,
                    nodes = { { id = "w", op = "white_noise" } }
                })"#,
            )
            .eval()
            .unwrap();
        let recipe = TextureRecipe::from_json(&json).expect("json parses");
        assert_eq!(recipe.resolution, 8);
        assert_eq!(recipe.seed, 2);
    }
}
