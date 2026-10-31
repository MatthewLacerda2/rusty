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
//! It is [`fingerprint`]: a content hash (`core::hash::content`) over the assembled
//! module, its params JSON and [`engine_hash`], the hash of every engine module a
//! baked shader can import ([`compose::MODULES`](super::compose::MODULES)), taken
//! once per process. The assembled module stands in for a shadergen version: the
//! base shader (`shader.wgsl`, `ui.wgsl`) and every block's WGSL are in it, so a
//! change to either moves the hash with no version number to remember to bump.
//!
//! Every loader of a baked module (surface, ui, postfx and the editor's shader
//! preview) reads it through [`super::baked::load`], which calls [`refresh`] first in
//! a build that may write (`dev` or `editor`); the default scene's seed calls it too.
//! [`refresh`] re-assembles the recipe against the engine's current shaders; when the
//! fingerprint no longer matches the module's first line, it re-bakes the module
//! from the recipe in place and logs one line naming it. So a renamed engine binding
//! (#1616) reaches every baked shader at its next load. A shipped player never
//! writes into its install: it re-composes a stale module in memory instead
//! ([`super::baked`]).
//!
//! What it leaves alone:
//! - a module with no `<name>.recipe.json` beside it (hand-written, or baked before
//!   #1638 wrote recipes): it is loaded as it is, and when it no longer composes the
//!   renderer logs that once and draws the standard shader, as before;
//! - a missing module, even with a recipe left beside it: a deleted shader stays
//!   deleted;
//! - a module whose recipe cannot be re-baked (a block since removed, a recipe naming
//!   another file): logged once per recipe and engine, and loaded as it is.
//!
//! A hand edit to a baked module survives until its recipe or the engine changes,
//! when the re-bake replaces it; deleting the recipe keeps the edit for good. An
//! edited recipe takes effect at the module's next load (a restart, or the next
//! write to the module), not live.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::bake::{assembled, bake_recipe};
use super::compose::MODULES;
use super::engine_shader_dir;
use super::recipe::ShaderRecipe;
use crate::core::collections::{Map, Set};
use crate::core::hash::content;

/// The first line of a baked module, before its fingerprint in hex.
const STAMP: &str = "// rusty engine fingerprint: ";

/// Whether this build re-bakes a stale module on disk. A shipped player
/// (`--no-default-features`) never writes into its install, so it re-composes in
/// memory instead (`shadergen::baked`).
pub const WRITES: bool = cfg!(any(feature = "dev", feature = "editor"));

/// `<dir>/<name>.recipe.json` for `<dir>/<name>.wgsl`: the recipe it was baked from.
pub fn recipe_path(wgsl: &Path) -> PathBuf {
    wgsl.with_extension("recipe.json")
}

/// The hash of the engine modules under `engine_shaders`, read once per process and
/// directory: the engine's shaders do not change under a running engine, and a bake
/// and a load must agree on it, or a module would be re-baked at every load.
pub fn engine_hash(engine_shaders: &str) -> Result<u64, String> {
    static HASHES: OnceLock<Mutex<Map<String, u64>>> = OnceLock::new();
    let hashes = HASHES.get_or_init(Mutex::default);
    if let Some(hash) = hashes
        .lock()
        .map_err(|e| e.to_string())?
        .get(engine_shaders)
    {
        return Ok(*hash);
    }
    let mut bytes = Vec::new();
    for name in MODULES {
        let path = format!("{engine_shaders}/{name}");
        let source =
            std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
        bytes.push(0);
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(source.replace("\r\n", "\n").as_bytes());
    }
    let hash = content(&bytes);
    let mut hashes = hashes.lock().map_err(|e| e.to_string())?;
    Ok(*hashes.entry(engine_shaders.to_owned()).or_insert(hash))
}

/// The fingerprint of `module` and its `params` JSON composed against engine
/// modules hashing to `engine`. Line endings are normalised, so a CRLF checkout agrees.
pub fn fingerprint(module: &str, params: &str, engine: u64) -> u64 {
    let mut bytes = module.replace("\r\n", "\n").into_bytes();
    bytes.push(0);
    bytes.extend_from_slice(params.replace("\r\n", "\n").as_bytes());
    bytes.extend_from_slice(&engine.to_le_bytes());
    content(&bytes)
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
    /// No module, or no recipe beside it: left alone.
    Unbaked,
    /// Baked against the engine's current shaders.
    Current,
    /// Stale, and re-baked from its recipe.
    Rebaked,
    /// Stale or unreadable, and could not be re-baked (logged once).
    Failed(String),
}

/// What a module's recipe says it should be now: what [`refresh`] writes, and what a
/// shipped player composes in memory.
pub(super) enum Plan {
    Unbaked,
    Current,
    /// Stale: the recipe, and the stamped module and params JSON it assembles to.
    Stale(ShaderRecipe, String, String),
}

/// Re-bake the module at `wgsl` from its recipe when it was baked against other
/// engine shaders than the ones this engine has; see the module doc.
pub fn refresh(wgsl: &Path) -> Refresh {
    refresh_against(wgsl, engine_shader_dir())
}

/// [`refresh`] against the engine shaders under `engine_shaders`.
pub fn refresh_against(wgsl: &Path, engine_shaders: &str) -> Refresh {
    let outcome = match plan(wgsl, engine_shaders) {
        Ok(Plan::Unbaked) => Refresh::Unbaked,
        Ok(Plan::Current) => Refresh::Current,
        Ok(Plan::Stale(recipe, ..)) => {
            let dir = wgsl.parent().unwrap_or(Path::new("."));
            match bake_recipe(&recipe, engine_shaders, &dir.to_string_lossy()) {
                Ok(_) => Refresh::Rebaked,
                Err(e) => Refresh::Failed(e.to_string()),
            }
        }
        Err(e) => Refresh::Failed(e),
    };
    let shown = wgsl.display();
    match &outcome {
        Refresh::Rebaked => {
            log::warn!("[Shader] {shown} was baked against older engine shaders; re-baked it from its recipe");
        }
        Refresh::Failed(e) => failed(wgsl, engine_shaders, e),
        Refresh::Unbaked | Refresh::Current => {}
    }
    outcome
}

/// Log a failed re-bake once per module, recipe and engine, not at every load.
pub(super) fn failed(wgsl: &Path, engine_shaders: &str, why: &str) {
    static LOGGED: OnceLock<Mutex<Set<(PathBuf, u64)>>> = OnceLock::new();
    let recipe = std::fs::read(recipe_path(wgsl)).unwrap_or_default();
    let key = (
        wgsl.to_path_buf(),
        content(&recipe) ^ engine_hash(engine_shaders).unwrap_or(0),
    );
    let fresh = LOGGED
        .get_or_init(Mutex::default)
        .lock()
        .is_ok_and(|mut logged| logged.insert(key));
    if fresh {
        log::warn!(
            "[Shader] {} could not be re-baked from its recipe: {why}",
            wgsl.display()
        );
    }
}

/// What the recipe beside `wgsl` assembles to now, against `engine_shaders`.
pub(super) fn plan(wgsl: &Path, engine_shaders: &str) -> Result<Plan, String> {
    let Some(have) = read_if_present(wgsl)? else {
        return Ok(Plan::Unbaked);
    };
    let Some(json) = read_if_present(&recipe_path(wgsl))? else {
        return Ok(Plan::Unbaked);
    };
    let recipe: ShaderRecipe =
        serde_json::from_str(&json).map_err(|e| format!("{}: {e}", recipe_path(wgsl).display()))?;
    let stem = wgsl.file_stem().unwrap_or_default().to_string_lossy();
    if recipe.name != stem {
        return Err(format!("its recipe is named {:?}", recipe.name));
    }
    let (module, params) = assembled(&recipe, engine_shaders)?;
    let want = fingerprint(&module, &params, engine_hash(engine_shaders)?);
    if recorded(&have) == Some(want) {
        return Ok(Plan::Current);
    }
    Ok(Plan::Stale(recipe, stamp(&module, want), params))
}

/// `path`'s text, or `None` when there is no such file.
fn read_if_present(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(test)]
#[path = "rebake_tests.rs"]
pub(super) mod tests;
