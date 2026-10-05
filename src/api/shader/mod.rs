//! src/api/shader/mod.rs — the `Shader` namespace (#272).
//!
//! Agent-facing surface for **WGSL shader authoring**: compose a recipe selecting
//! a base pass (`surface` | `postfx` | `ui`) and a list of curated building blocks, then
//! **bake** a `.wgsl` module that conforms to the engine's pass + bind-group
//! contract. The bake **validates the assembled module by composing it through
//! `naga_oil`** (the same path the engine loads it by) and **rejects a module that
//! won't compile — no file is written** — so a bad shader never ships. On success
//! the module is written to the authored-shader workspace, registered by name (a
//! `ShaderRegistry` pointed at that dir loads `<name>.wgsl`).
//!
//! The recipe is authored as a Lua table whose shape mirrors the serde document
//! exactly (see `from_lua`), so the same recipe describes a Lua-built shader and
//! one loaded from `.json` — one surface, three callers. Every verb taking a
//! `recipe` accepts either form: the table or its JSON string (#410).
//!
//! Verbs:
//! - `Shader.Bake(recipe [, out_dir])` — assemble + validate + write; returns the
//!   written path. `out_dir` defaults to the project shader workspace.
//! - `Shader.Validate(recipe)` — assemble + compose-check **without writing**;
//!   returns the composed entry-point names (the dry-run gate the agent checks a
//!   recipe with before baking).
//! - `Shader.ToJson(recipe)` — serialize a recipe to canonical JSON.
//! - `Shader.Blocks(pass)` — list the curated blocks for a pass with their
//!   descriptions and params (name, default, arity, runtime) — introspection so the
//!   agent composes from the actual catalog, not guesswork (#411).
//!
//! `Shader` borrows no engine state — it reads a recipe and writes a file — so it
//! registers as a plain static namespace, like `Texture`.

mod blocks;
mod from_lua;

use mlua::{Lua, Value};

use super::{put, Reg};
use crate::shadergen::assemble::assemble;
use crate::shadergen::bake::base_source;
use crate::shadergen::recipe::PassKind;
use crate::shadergen::validate::validate;
use crate::shadergen::{bake_recipe, engine_shader_dir, ShaderRecipe, DEFAULT_OUT_DIR};
use from_lua::parse_recipe;

/// Register the `Shader` namespace onto `lua`.
pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Bake",
        lua.create_function(|_, (recipe, out_dir): (Value, Option<String>)| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            bake(&recipe, out_dir)
        }),
    )?;

    put(
        &table,
        "Validate",
        lua.create_function(|lua, recipe: Value| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            let entries = dry_run(&recipe).map_err(mlua::Error::RuntimeError)?;
            lua.create_sequence_from(entries)
        }),
    )?;

    put(
        &table,
        "ToJson",
        lua.create_function(|_, recipe: Value| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            recipe.to_json().map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        &table,
        "Blocks",
        lua.create_function(|lua, pass: String| {
            let pass = parse_pass(&pass).map_err(mlua::Error::RuntimeError)?;
            blocks::blocks_table(lua, pass)
        }),
    )?;

    lua.globals()
        .set("Shader", table)
        .map_err(|e| e.to_string())
}

/// Assemble + validate + write a parsed recipe, surfacing any error to Lua. The
/// engine shader dir is fixed (read-only base + `common`); `out_dir` defaults to
/// the project's `assets/shaders`.
fn bake(recipe: &ShaderRecipe, out_dir: Option<String>) -> mlua::Result<String> {
    let out = out_dir.as_deref().unwrap_or(DEFAULT_OUT_DIR);
    bake_recipe(recipe, engine_shader_dir(), out)
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))
}

/// Assemble + compose-check a recipe without writing; returns the entry points.
fn dry_run(recipe: &ShaderRecipe) -> Result<Vec<String>, String> {
    let base = base_source(recipe.pass, engine_shader_dir())?;
    let module = assemble(recipe, &base)?;
    validate(engine_shader_dir(), &module)
}

/// Parse a pass tag from Lua (`"surface"` | `"postfx"` | `"ui"`).
fn parse_pass(name: &str) -> Result<PassKind, String> {
    let lower = name.to_ascii_lowercase();
    PassKind::ALL
        .into_iter()
        .find(|p| p.tag() == lower)
        .ok_or_else(|| format!("unknown shader pass: {lower} (want surface | postfx | ui)"))
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::*;

    #[test]
    fn bake_writes_a_composing_postfx_module() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let dir = crate::test_temp::dir().join("rusty_shader_api_bake");
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
    fn blocks_describes_each_block_and_its_params() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let (id, name, default, arity, runtime): (String, String, f32, usize, bool) = lua
            .load(
                r#"for _, b in ipairs(Shader.Blocks("postfx")) do
                     if b.id == "vignette" then
                       local p = b.params[1]
                       return b.id, p.name, p.default, p.arity, p.runtime
                     end
                   end"#,
            )
            .eval()
            .unwrap();
        let block = crate::shadergen::blocks::find(PassKind::Postfx, "vignette").unwrap();
        assert_eq!(
            (id.as_str(), name.as_str()),
            ("vignette", block.params[0].name)
        );
        assert_eq!(
            (default, arity, runtime),
            (block.params[0].default, 1, true)
        );
    }

    #[test]
    fn a_vector_default_comes_back_as_an_array_of_its_arity() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let lanes: usize = lua
            .load(
                r#"for _, b in ipairs(Shader.Blocks("surface")) do
                     if b.id == "tint" then return #b.params[1].default end
                   end"#,
            )
            .eval()
            .unwrap();
        assert_eq!(lanes, 3);
    }

    #[test]
    fn blocks_list_runtime_params_and_the_texture_slots_they_sample() {
        let lua = Lua::new();
        register(&lua).unwrap();
        let (slot, runtime, none, stages): (String, bool, usize, String) = lua
            .load(
                r#"local d, t, u
                   for _, b in ipairs(Shader.Blocks("surface")) do
                     if b.id == "dissolve" then d = b end
                     if b.id == "tint" then t = b end
                     if b.id == "uv_scroll" then u = b end
                   end
                   return d.textures[1], d.params[1].runtime, #t.textures, t.stage .. u.stage"#,
            )
            .eval()
            .unwrap();
        assert_eq!((slot.as_str(), runtime, none), ("mask", true, 0));
        assert_eq!(stages, "coloruv");
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
