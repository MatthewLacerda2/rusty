//! Each bake checks its size before it makes a folder or readies a sidecar, so a refused bake
//! writes nothing.

mod folder;
mod from_lua;
mod inspect;
mod mesh_maps;
mod ops;

use std::collections::BTreeMap;
use std::path::Path;

use mlua::{Lua, Value};

use super::{put, Reg};
use from_lua::parse_recipe;
use rusty_asset::sidecar::{check_bake_size, prepare_bake};
use rusty_core::project::ContentRoots;
use rusty_edit::authoring::material::{load_recipe, Loaded};
use rusty_procgen::{bake_recipe, bake_set, check_set, set_paths, Slot, TextureRecipe};

pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Bake",
        lua.create_function(|_, (recipe, path, slot): (Value, String, String)| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            bake(&recipe, &path, &slot, &rusty_core::project::root())
        }),
    )?;

    put(
        &table,
        "BakeSet",
        lua.create_function(|lua, (recipe, prefix): (Value, String)| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            let root = rusty_core::project::root();
            lua.create_table_from(bake_whole_set(&recipe, &prefix, &root)?)
        }),
    )?;

    mesh_maps::register(lua, &table)?;

    put(
        &table,
        "ToJson",
        lua.create_function(|_, recipe: Value| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            recipe.to_json().map_err(mlua::Error::RuntimeError)
        }),
    )?;

    inspect::register(lua, &table)?;

    put(
        &table,
        "Ops",
        lua.create_function(|lua, ()| {
            ops::ops_table(lua, &ContentRoots::new(&rusty_core::project::root()))
        }),
    )?;

    lua.globals()
        .set("Texture", table)
        .map_err(|e| e.to_string())
}

fn bake(recipe: &TextureRecipe, path: &str, slot: &str, root: &Path) -> mlua::Result<String> {
    let slot = Slot::parse(slot).map_err(mlua::Error::RuntimeError)?;
    let loaded = load(recipe)?;
    let size = loaded.recipe.extent();
    let extent = (size.width, size.height);
    check_bake_size(extent)
        .and_then(|()| folder::make_parents(path, root))
        .and_then(|()| prepare_bake("Texture.Bake", extent, [path]))
        .and_then(|()| bake_recipe(&loaded.recipe, &loaded.sources, slot, path))
        .map_err(|e| mlua::Error::RuntimeError(format!("Texture.Bake: {e}")))
}

fn bake_whole_set(
    recipe: &TextureRecipe,
    prefix: &str,
    root: &Path,
) -> mlua::Result<BTreeMap<String, String>> {
    let loaded = load_set(recipe)?;
    let size = loaded.recipe.extent();
    let paths = set_paths(&loaded.recipe, prefix).map_err(mlua::Error::RuntimeError)?;
    let extent = (size.width, size.height);
    check_bake_size(extent)
        .and_then(|()| folder::make_parents(prefix, root))
        .and_then(|()| prepare_bake("Texture.BakeSet", extent, paths.iter().map(String::as_str)))
        .and_then(|()| bake_set(&loaded.recipe, &loaded.sources, prefix))
        .map_err(|e| mlua::Error::RuntimeError(format!("Texture.BakeSet: {e}")))
}

fn load_set(recipe: &TextureRecipe) -> mlua::Result<Loaded> {
    check_set(recipe).map_err(mlua::Error::RuntimeError)?;
    load(recipe)
}

fn load(recipe: &TextureRecipe) -> mlua::Result<Loaded> {
    load_recipe(recipe, &ContentRoots::new(&rusty_core::project::root()))
        .map_err(mlua::Error::RuntimeError)
}

#[cfg(test)]
mod bake_set_tests;
#[cfg(test)]
mod bake_size_tests;
#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod inspect_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod size_tests;
#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn bake_writes_a_loadable_png() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let path = rusty_test_support::temp::dir().join("rusty_texture_api_bake.png");
        let p = path.to_str().unwrap();
        // Pass the path as a Lua global *value*, not interpolated into the Lua
        // source, so no character of it is read as Lua syntax.
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
        let recipe: TextureRecipe = crate::api::lua_json::recipe_from_lua(&mlua::Value::String(
            lua.create_string(&json).unwrap(),
        ))
        .expect("json parses");
        assert_eq!(recipe.resolution, 8);
        assert_eq!(recipe.seed, 2);
    }

    #[test]
    fn an_unknown_slot_fails_the_bake_before_writing() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let path = rusty_test_support::temp::dir().join("rusty_texture_api_bad_slot.png");
        std::fs::remove_file(&path).ok();
        lua.globals().set("OUT", path.to_str().unwrap()).unwrap();
        let script = r#"
            return Texture.Bake({ resolution = 8, nodes = { { id = "w", op = "white_noise" } } },
              OUT, "basecolour")
        "#;
        let err = lua.load(script).eval::<String>().unwrap_err().to_string();
        assert!(err.contains("unknown slot \"basecolour\""), "{err}");
        assert!(!path.exists(), "a refused bake writes nothing");
    }
}
