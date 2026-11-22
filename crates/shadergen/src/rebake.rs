use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::bake::{assembled, bake_recipe};
use super::compose::MODULES;
use super::engine_shader_dir;
use super::recipe::ShaderRecipe;
use rusty_core::collections::{Map, Set};
use rusty_core::hash::content;

const STAMP: &str = "// rusty engine fingerprint: ";

pub const WRITES: bool = cfg!(any(feature = "dev", feature = "editor"));

pub fn recipe_path(wgsl: &Path) -> PathBuf {
    wgsl.with_extension("recipe.json")
}

pub fn engine_hash(engine_shaders: &str) -> Result<u64, String> {
    // A bake and a load must agree on it, or a module is re-baked at every load.
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

pub fn fingerprint(module: &str, params: &str, engine: u64) -> u64 {
    let mut bytes = module.replace("\r\n", "\n").into_bytes();
    bytes.push(0);
    bytes.extend_from_slice(params.replace("\r\n", "\n").as_bytes());
    bytes.extend_from_slice(&engine.to_le_bytes());
    content(&bytes)
}

pub fn stamp(module: &str, fingerprint: u64) -> String {
    format!("{STAMP}{fingerprint:016x}\n{module}")
}

fn recorded(wgsl: &str) -> Option<u64> {
    let hex = wgsl.lines().next()?.strip_prefix(STAMP)?;
    u64::from_str_radix(hex.trim(), 16).ok()
}

#[derive(Debug, PartialEq)]
pub enum Refresh {
    Unbaked,
    Current,
    Rebaked,
    Failed(String),
}

pub(super) enum Plan {
    Unbaked,
    Current,
    Stale(ShaderRecipe, String, String),
}

pub fn refresh(wgsl: &Path) -> Refresh {
    refresh_against(wgsl, engine_shader_dir())
}

pub fn refresh_all(dir: &Path) -> Vec<PathBuf> {
    let mut modules = Vec::new();
    wgsl_under(dir, &mut modules);
    modules.sort();
    modules.retain(|wgsl| refresh(wgsl) == Refresh::Rebaked);
    modules
}

fn wgsl_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            wgsl_under(&path, found);
        } else if path.extension().is_some_and(|e| e == "wgsl") {
            found.push(path);
        }
    }
}

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

fn read_if_present(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(test)]
#[path = "rebake_tests.rs"]
pub(crate) mod tests;
