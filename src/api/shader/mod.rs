//! src/api/shader/mod.rs — the `Shader` namespace (#272).
//!
//! Agent-facing surface for **WGSL shader authoring**: compose a recipe selecting
//! a base pass (`surface` | `postfx`) and a list of curated building blocks, then
//! **bake** a `.wgsl` module that conforms to the engine's pass + bind-group
//! contract. The bake **validates the assembled module by composing it through
//! `naga_oil`** (the same path the engine loads it by) and **rejects a module that
//! won't compile — no file is written** — so a bad shader never ships. On success
//! the module is written to the authored-shader workspace, registered by name (a
//! `ShaderRegistry` pointed at that dir loads `<name>.wgsl`).
//!
//! The recipe is authored as a Lua table whose shape mirrors the serde document
//! exactly (see [`from_lua`]), so the same recipe describes a Lua-built shader and
//! one loaded from `.json` — one surface, three callers.
//!
//! Verbs:
//! - `Shader.Bake(recipe [, out_dir])` — assemble + validate + write; returns the
//!   written path. `out_dir` defaults to the project shader workspace.
//! - `Shader.BakeJson(json [, out_dir])` — same, from a recipe JSON string.
//! - `Shader.Validate(recipe)` — assemble + compose-check **without writing**;
//!   returns the composed entry-point names (the dry-run gate the agent checks a
//!   recipe with before baking).
//! - `Shader.ToJson(recipe)` — serialize a recipe table to canonical JSON.
//! - `Shader.Blocks(pass)` — list the curated block ids for a pass (introspection
//!   so the agent composes from the actual catalog, not guesswork).
//!
//! `Shader` borrows no engine state — it reads a recipe and writes a file — so it
//! registers as a plain static namespace, like `Texture`.

mod from_lua;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::shadergen::assemble::assemble;
use crate::shadergen::blocks::catalog;
use crate::shadergen::recipe::PassKind;
use crate::shadergen::validate::validate;
use crate::shadergen::{bake_recipe, ShaderRecipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};
use from_lua::recipe_from_table;

/// Register the `Shader` namespace onto `lua`.
pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Bake",
        lua.create_function(|_, (recipe, out_dir): (Table, Option<String>)| {
            let recipe = recipe_from_table(&recipe).map_err(mlua::Error::RuntimeError)?;
            bake(&recipe, out_dir)
        }),
    )?;

    put(
        &table,
        "BakeJson",
        lua.create_function(|_, (json, out_dir): (String, Option<String>)| {
            let recipe = ShaderRecipe::from_json(&json).map_err(mlua::Error::RuntimeError)?;
            bake(&recipe, out_dir)
        }),
    )?;

    put(
        &table,
        "Validate",
        lua.create_function(|lua, recipe: Table| {
            let recipe = recipe_from_table(&recipe).map_err(mlua::Error::RuntimeError)?;
            let entries = dry_run(&recipe).map_err(mlua::Error::RuntimeError)?;
            lua.create_sequence_from(entries)
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

    put(
        &table,
        "Blocks",
        lua.create_function(|lua, pass: String| {
            let pass = parse_pass(&pass).map_err(mlua::Error::RuntimeError)?;
            let ids: Vec<String> = catalog(pass).iter().map(|b| b.id.to_string()).collect();
            lua.create_sequence_from(ids)
        }),
    )?;

    lua.globals()
        .set("Shader", table)
        .map_err(|e| e.to_string())
}

/// Assemble + validate + write a parsed recipe, surfacing any error to Lua. The
/// engine shader dir is fixed (read-only base + `common`); `out_dir` defaults to
/// the project workspace.
fn bake(recipe: &ShaderRecipe, out_dir: Option<String>) -> mlua::Result<String> {
    let out = out_dir.as_deref().unwrap_or(DEFAULT_OUT_DIR);
    bake_recipe(recipe, ENGINE_SHADER_DIR, out)
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))
}

/// Assemble + compose-check a recipe without writing; returns the entry points.
fn dry_run(recipe: &ShaderRecipe) -> Result<Vec<String>, String> {
    let surface_base = match recipe.pass {
        PassKind::Surface => std::fs::read_to_string(format!("{ENGINE_SHADER_DIR}/shader.wgsl"))
            .map_err(|e| e.to_string())?,
        PassKind::Postfx => String::new(),
    };
    let module = assemble(recipe, &surface_base)?;
    validate(ENGINE_SHADER_DIR, &module)
}

/// Parse a pass tag from Lua (`"surface"` | `"postfx"`).
fn parse_pass(name: &str) -> Result<PassKind, String> {
    match name.to_ascii_lowercase().as_str() {
        "surface" => Ok(PassKind::Surface),
        "postfx" => Ok(PassKind::Postfx),
        other => Err(format!(
            "unknown shader pass: {other} (want surface | postfx)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn bake_writes_a_composing_postfx_module() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let dir = std::env::temp_dir().join("rusty_shader_api_bake");
        // Pass the path as a Lua global *value*, not interpolated into the Lua
        // source: a Windows path embedded in a string literal trips Lua's escape
        // parser (`\U`). As a global it's a plain string, safe on every platform.
        lua.globals().set("OUT", dir.to_str().unwrap()).unwrap();
        let script = r#"
            return Shader.Bake({
              pass = "postfx", name = "api_warm",
              blocks = { { id = "tint", params = { color = {1.0, 0.8, 0.6} } } }
            }, OUT)
        "#;
        let path: String = lua.load(script).eval().unwrap();
        assert!(path.ends_with("api_warm.wgsl"), "got {path}");
        let src = std::fs::read_to_string(&path).expect("baked file exists");
        assert!(src.contains("fn fs_main"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn validate_returns_entry_points_without_writing() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let entries: Vec<String> = lua
            .load(
                r#"return Shader.Validate({
                    pass = "postfx", name = "dry",
                    blocks = { { id = "grayscale" } }
                })"#,
            )
            .eval()
            .unwrap();
        assert!(entries.iter().any(|e| e == "fs_main"), "{entries:?}");
    }

    #[test]
    fn blocks_lists_the_catalog_for_a_pass() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let ids: Vec<String> = lua
            .load(r#"return Shader.Blocks("postfx")"#)
            .eval()
            .unwrap();
        assert!(ids.iter().any(|i| i == "vignette"), "{ids:?}");
    }

    #[test]
    fn to_json_round_trips_back_to_a_recipe() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let json: String = lua
            .load(
                r#"return Shader.ToJson({
                    pass = "surface", name = "toon",
                    blocks = { { id = "toon_ramp", params = { steps = 3.0 } } }
                })"#,
            )
            .eval()
            .unwrap();
        let recipe = ShaderRecipe::from_json(&json).expect("json parses");
        assert_eq!(recipe.name, "toon");
        assert_eq!(recipe.blocks[0].id, "toon_ramp");
    }
}
