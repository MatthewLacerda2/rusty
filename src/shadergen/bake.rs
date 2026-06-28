//! src/shadergen/bake.rs — the one-call front door: assemble → validate → write
//! (#272).
//!
//! [`bake_recipe`] is the verb the API surface drives. It:
//! 1. reads the canonical forward shader from `engine_shaders` (for a surface
//!    recipe — the base whose `vs_main`/lighting the variant keeps),
//! 2. [`assemble`](super::assemble)s the recipe into a complete `.wgsl` module,
//! 3. [`validate`](super::validate)s it by composing through `naga_oil` against
//!    `common` (the gate — a module that won't compose is **never written**), and
//! 4. writes `<out_dir>/<name>.wgsl`, the file the engine's `ShaderRegistry` loads
//!    by name.
//!
//! The engine's committed shader set (`engine_shaders`, normally
//! `assets/shaders`) is read-only here — it supplies the surface base and the
//! `common` module to validate against. The baked output goes to a separate
//! `out_dir` (the authored-content workspace, e.g. `project/assets/shaders`), so a
//! bake never mutates the shipped set. For the engine to *load* a baked variant,
//! point a `ShaderRegistry` at `out_dir` (it reads `<base>/<name>.wgsl`); validate
//! used the engine's `common`, so the file composes there identically.

use super::assemble::assemble;
use super::recipe::{PassKind, ShaderRecipe};
use super::validate::validate;

/// Why a bake failed — surfaced verbatim to the caller (the Lua error / REPL).
#[derive(Debug)]
pub enum BakeError {
    /// Reading the surface base shader from the engine shader dir failed.
    BaseRead(String),
    /// Assembling the recipe into WGSL failed (e.g. unknown block).
    Assemble(String),
    /// The assembled module failed to compose through `naga_oil` — rejected so a
    /// bad shader never reaches disk.
    Validate(String),
    /// Writing the validated module to the output dir failed.
    Write(String),
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::BaseRead(m) => write!(f, "shader bake: reading surface base failed: {m}"),
            BakeError::Assemble(m) => write!(f, "shader bake: assembly failed: {m}"),
            BakeError::Validate(m) => write!(f, "shader bake: validation failed: {m}"),
            BakeError::Write(m) => write!(f, "shader bake: writing module failed: {m}"),
        }
    }
}

impl std::error::Error for BakeError {}

/// Assemble + validate + write `recipe` to `<out_dir>/<name>.wgsl`. `engine_shaders`
/// is the engine's committed shader dir (read-only — supplies the surface base and
/// the `common` module to validate against). Returns the written path on success;
/// a validation failure rejects the bake with the composer's error and **no file
/// is written**.
pub fn bake_recipe(
    recipe: &ShaderRecipe,
    engine_shaders: &str,
    out_dir: &str,
) -> Result<String, BakeError> {
    let surface_base = match recipe.pass {
        PassKind::Surface => std::fs::read_to_string(format!("{engine_shaders}/shader.wgsl"))
            .map_err(|e| BakeError::BaseRead(e.to_string()))?,
        PassKind::Postfx => String::new(),
    };

    let module = assemble(recipe, &surface_base).map_err(BakeError::Assemble)?;
    validate(engine_shaders, &module).map_err(BakeError::Validate)?;

    std::fs::create_dir_all(out_dir).map_err(|e| BakeError::Write(e.to_string()))?;
    let path = format!("{out_dir}/{}.wgsl", recipe.name);
    std::fs::write(&path, &module).map_err(|e| BakeError::Write(e.to_string()))?;
    Ok(path)
}
