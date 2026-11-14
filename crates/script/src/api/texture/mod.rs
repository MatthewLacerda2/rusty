//! crates/script/src/api/texture/mod.rs — the `Texture` namespace (#270).
//!
//! Agent-facing surface for procedural texture authoring: compose a recipe of ops and
//! bake it to a tiling `.png` a material map slot consumes. The recipe is authored as
//! a Lua table whose shape mirrors the serde document exactly (see
//! `from_lua`), so the same DAG describes a Lua-built
//! recipe and one loaded from `.json` — one surface, three callers.
//!
//! Verbs:
//! - `Texture.Bake(recipe, path, slot)` — evaluate a recipe and write the PNG,
//!   encoded for the named `slot` (base_color / emissive = sRGB; normal /
//!   roughness / metallic / data = linear; `metallic_roughness` packs metallic→B,
//!   roughness→G). Returns the written path.
//! - `Texture.BakeSet(recipe, prefix)` — bake every entry of the recipe's
//!   `outputs` (slot → node id) from one evaluation to `<prefix>_<slot>.png`
//!   (#403). Returns a table canonical slot → path.
//! - `Texture.BakeMeshMaps(model, maps, prefix, [resolution])` — bake a model's
//!   mesh maps (curvature, AO, world normal, position, thickness, ID) in its UV0 to
//!   `<prefix><map>.png` (`mesh_maps`, #1849). Returns a table map → path.
//! - `Texture.ToJson(recipe)` — serialize a recipe to its canonical JSON for
//!   saving / diffing.
//! - `Texture.Stats(recipe, [resolution])` — every node's min / max / mean / std /
//!   p1 / p99 per channel, from one evaluation (#1486).
//! - `Texture.Preview(recipe, path, [size])` — a PNG contact sheet of every node's
//!   image, with the grid's column count and node order (#1486).
//! - `Texture.Ops()` — the op catalog: each op's tag, category, input count and
//!   params with their types and defaults (#411), so the agent discovers the op-set
//!   from the engine rather than the docs; then the project's `.graph.json` graphs,
//!   each as `op = "graph"` with its path, inputs and params (#1744).
//!
//! A recipe's graph instances (#1744) and `image` nodes read their files from the
//! project root before any verb evaluates it (`scene::authoring::material::load_recipe`,
//! #1491): procgen itself reads no files. Each verb evaluates the expanded recipe,
//! so `Stats` and `Preview` show an instance's nodes under their prefixed ids
//! (`brick/mortar`); `ToJson` writes the recipe as given, instances unexpanded.
//!
//! `Bake` and `BakeSet` make the missing folders above their target inside the
//! project (`folder`, #1850), and a failed write names the verb and the path.
//!
//! A `recipe` is a Lua table **or** its JSON string (the on-disk form, #410), so a
//! saved recipe re-bakes through the same verb without a round-trip through Lua.
//!
//! An unknown `slot`, or an unknown key anywhere in a recipe, is an error naming it
//! (#395) — a typo never bakes a silently wrong map. So is a link (#1494) to a node
//! that doesn't exist, one that closes a cycle, or one on a param that takes none.
//!
//! `Texture` borrows no engine state — it reads a recipe and writes a file — so it
//! registers as a plain static namespace, like `Assets`.

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
use rusty_procgen::{bake_recipe, bake_set, check_set, Slot, TextureRecipe};
use rusty_scene::authoring::material::{load_recipe, Loaded};

/// Register the `Texture` namespace onto `lua`.
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
        lua.create_function(|lua, ()| ops::ops_table(lua, &rusty_core::project::root())),
    )?;

    lua.globals()
        .set("Texture", table)
        .map_err(|e| e.to_string())
}

/// Evaluate + bake a parsed recipe for the named slot, surfacing any error to Lua.
/// The missing folders above `path` are made first when they lie inside `root`.
fn bake(recipe: &TextureRecipe, path: &str, slot: &str, root: &Path) -> mlua::Result<String> {
    let slot = Slot::parse(slot).map_err(mlua::Error::RuntimeError)?;
    let loaded = load(recipe)?;
    folder::make_parents(path, root)
        .and_then(|()| bake_recipe(&loaded.recipe, &loaded.sources, slot, path))
        .map_err(|e| mlua::Error::RuntimeError(format!("Texture.Bake: {e}")))
}

/// `Texture.BakeSet`'s body: every output of `recipe` to `<prefix>_<slot>.png`,
/// the folder above `prefix` made first when it lies inside `root`.
fn bake_whole_set(
    recipe: &TextureRecipe,
    prefix: &str,
    root: &Path,
) -> mlua::Result<BTreeMap<String, String>> {
    let loaded = load_set(recipe)?;
    folder::make_parents(prefix, root)
        .and_then(|()| bake_set(&loaded.recipe, &loaded.sources, prefix))
        .map_err(|e| mlua::Error::RuntimeError(format!("Texture.BakeSet: {e}")))
}

/// [`load`] for a set bake: its `outputs` are checked first, so a recipe with
/// none says so rather than naming a file it couldn't read.
fn load_set(recipe: &TextureRecipe) -> mlua::Result<Loaded> {
    check_set(recipe).map_err(mlua::Error::RuntimeError)?;
    load(recipe)
}

/// `recipe` ready to run: its graphs expanded and its images decoded, from the
/// project root.
fn load(recipe: &TextureRecipe) -> mlua::Result<Loaded> {
    load_recipe(recipe, &rusty_core::project::root()).map_err(mlua::Error::RuntimeError)
}

#[cfg(test)]
mod bake_set_tests;
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
