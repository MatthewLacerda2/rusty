//! src/shadergen/rebake.rs — a baked shader follows the engine it was baked
//! against (#1638): Unity recompiles a shader when its includes change, and the user
//! never re-bakes by hand.
//!
//! A bake ([`super::bake_recipe`]) writes three files: the module `<name>.wgsl`,
//! its params `<name>.params.json` and its recipe `<name>.recipe.json`. The module's
//! first line is a **fingerprint** of what it was composed from:
//!
//! ```text
//! // rusty engine fingerprint: 3f0c9a1d5e7b2468
//! ```
//!
//! It is [`fingerprint`]: an FNV-1a hash (`core::hash`) over the assembled module and
//! the source of every engine module a baked shader can import
//! ([`compose::MODULES`](super::compose::MODULES)). The assembled module stands in
//! for a shadergen version: the base shader (`shader.wgsl`, `ui.wgsl`) and every
//! block's WGSL are in it, so a change to either moves the hash with no version
//! number to remember to bump.
//!
//! [`refresh`] runs when a renderer loads a baked module (a surface, ui or postfx
//! variant) and when the default scene's shader is seeded. It re-assembles the
//! recipe against the engine's current shaders; when the fingerprint no longer
//! matches the module's first line, it re-bakes the module from the recipe in place
//! and logs one line naming it. So a renamed engine binding (#1616) or a new entry
//! point (#1526) reaches every baked shader at its next load.
//!
//! What it leaves alone:
//! - a module with no `<name>.recipe.json` beside it (hand-written, or baked before
//!   #1638 wrote recipes): it is loaded as it is, and when it no longer composes the
//!   renderer logs that once and draws the standard shader, as before;
//! - a module whose recipe cannot be re-baked (a block since removed, a recipe naming
//!   another file): logged, and loaded as it is.
//!
//! A hand edit to a baked module survives until its recipe or the engine changes,
//! when the re-bake replaces it; deleting the recipe keeps the edit for good. Editing
//! the recipe re-bakes the module at its next load.

use std::path::{Path, PathBuf};

use super::assemble::assemble_with_params;
use super::bake::{bake_recipe, base_source};
use super::compose::MODULES;
use super::engine_shader_dir;
use super::recipe::ShaderRecipe;
use crate::core::hash::fnv1a64;

/// The first line of a baked module, before its fingerprint in hex.
const STAMP: &str = "// rusty engine fingerprint: ";

/// `<dir>/<name>.recipe.json` for `<dir>/<name>.wgsl`: the recipe it was baked from.
pub fn recipe_path(wgsl: &Path) -> PathBuf {
    wgsl.with_extension("recipe.json")
}

/// The fingerprint of `module` composed against the engine modules under
/// `engine_shaders`. Line endings are normalised, so a CRLF checkout agrees.
pub fn fingerprint(module: &str, engine_shaders: &str) -> Result<u64, String> {
    let mut bytes = module.replace("\r\n", "\n").into_bytes();
    for name in MODULES {
        let path = format!("{engine_shaders}/{name}");
        let source =
            std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
        bytes.push(0);
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(source.replace("\r\n", "\n").as_bytes());
    }
    Ok(fnv1a64(&bytes))
}

/// `module` with its fingerprint line on top: what a bake writes.
pub fn stamp(module: &str, fingerprint: u64) -> String {
    format!("{STAMP}{fingerprint:016x}\n{module}")
}

/// The fingerprint a baked module's first line records, if it has one.
fn recorded(wgsl: &str) -> Option<u64> {
    let hex = wgsl.lines().next()?.strip_prefix(STAMP)?;
    u64::from_str_radix(hex.trim(), 16).ok()
}

/// What [`refresh`] found.
#[derive(Debug, PartialEq)]
pub enum Refresh {
    /// No recipe beside the module: hand-written, left alone.
    Unbaked,
    /// Baked against the engine's current shaders.
    Current,
    /// Stale, and re-baked from its recipe.
    Rebaked,
    /// Stale or unreadable, and could not be re-baked (logged).
    Failed(String),
}

/// Re-bake the module at `wgsl` from its recipe when it was baked against other
/// engine shaders than the ones this engine has; see the module doc.
pub fn refresh(wgsl: &Path) -> Refresh {
    refresh_against(wgsl, engine_shader_dir())
}

/// [`refresh`] against the engine shaders under `engine_shaders`.
pub fn refresh_against(wgsl: &Path, engine_shaders: &str) -> Refresh {
    let outcome = check(wgsl, engine_shaders).unwrap_or_else(Refresh::Failed);
    let shown = wgsl.display();
    match &outcome {
        Refresh::Rebaked => {
            log::warn!("[Shader] {shown} was baked against older engine shaders; re-baked it from its recipe");
        }
        Refresh::Failed(e) => {
            log::warn!("[Shader] {shown} could not be re-baked from its recipe: {e}");
        }
        Refresh::Unbaked | Refresh::Current => {}
    }
    outcome
}

fn check(wgsl: &Path, engine_shaders: &str) -> Result<Refresh, String> {
    let json = match std::fs::read_to_string(recipe_path(wgsl)) {
        Ok(json) => json,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Refresh::Unbaked),
        Err(e) => return Err(e.to_string()),
    };
    let recipe: ShaderRecipe =
        serde_json::from_str(&json).map_err(|e| format!("{}: {e}", recipe_path(wgsl).display()))?;
    let stem = wgsl.file_stem().unwrap_or_default().to_string_lossy();
    if recipe.name != stem {
        return Err(format!("its recipe is named {:?}", recipe.name));
    }
    let base = base_source(recipe.pass, engine_shaders)?;
    let (module, _) = assemble_with_params(&recipe, &base)?;
    let want = fingerprint(&module, engine_shaders)?;
    let have = std::fs::read_to_string(wgsl).ok();
    if have.as_deref().and_then(recorded) == Some(want) {
        return Ok(Refresh::Current);
    }
    let dir = wgsl.parent().unwrap_or(Path::new("."));
    bake_recipe(&recipe, engine_shaders, &dir.to_string_lossy()).map_err(|e| e.to_string())?;
    Ok(Refresh::Rebaked)
}

#[cfg(test)]
#[path = "rebake_tests.rs"]
mod tests;
