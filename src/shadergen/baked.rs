//! src/shadergen/baked.rs — the one loader of an authored shader module (#1638):
//! every renderer that reads a `<name>.wgsl` and its params (surface, ui, postfx and
//! the editor's shader preview) reads it through [`load`], so a stale baked module is
//! brought up to date in one place.
//!
//! In a build that may write (`dev` or `editor`, [`rebake::WRITES`]) a stale module
//! is re-baked on disk from its recipe first ([`rebake::refresh`]). A shipped player
//! never writes into its install (a signed `.app`, a read-only directory): it
//! re-composes the stale module from its recipe **in memory** and loads that, leaving
//! the files as shipped. An export (`dev::export`) re-bakes every stale module before
//! it copies ([`rebake::refresh_all`]), so an exported game's modules load as they
//! are; the in-memory re-compose is the fallback for a project the player runs
//! without an export (a project folder copied by hand, or one exported against
//! older engine shaders than the player ships beside).

use std::path::Path;

use super::engine_shader_dir;
use super::params::{sidecar_path, ParamLayout};
use super::rebake::{self, Plan};

/// A module's source and its params JSON (`None` when it has no sidecar, as a
/// module baked before #399 has none).
#[derive(Debug, Clone, PartialEq)]
pub struct Baked {
    pub source: String,
    pub params: Option<String>,
}

impl Baked {
    /// The runtime params; `wgsl` names the module in a parse error.
    pub fn layout(&self, wgsl: &Path) -> Result<ParamLayout, String> {
        let Some(json) = &self.params else {
            return Ok(ParamLayout::default());
        };
        serde_json::from_str(json).map_err(|e| format!("{}: {e}", sidecar_path(wgsl).display()))
    }
}

/// Load the module at `wgsl`, brought up to date with the engine's shaders.
pub fn load(wgsl: &Path) -> Result<Baked, String> {
    load_against(wgsl, engine_shader_dir(), rebake::WRITES)
}

/// [`load`] against `engine_shaders`; `writes` says whether a stale module is
/// re-baked on disk or re-composed in memory.
pub fn load_against(wgsl: &Path, engine_shaders: &str, writes: bool) -> Result<Baked, String> {
    if writes {
        rebake::refresh_against(wgsl, engine_shaders);
    } else {
        match rebake::plan(wgsl, engine_shaders) {
            Ok(Plan::Stale(_, source, params)) => {
                return Ok(Baked {
                    source,
                    params: Some(params),
                })
            }
            Ok(Plan::Unbaked | Plan::Current) => {}
            Err(e) => rebake::failed(wgsl, engine_shaders, &e),
        }
    }
    let source = std::fs::read_to_string(wgsl).map_err(|e| format!("{}: {e}", wgsl.display()))?;
    let params = match std::fs::read_to_string(sidecar_path(wgsl)) {
        Ok(json) => Some(json),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", sidecar_path(wgsl).display())),
    };
    Ok(Baked { source, params })
}

#[cfg(test)]
#[path = "baked_tests.rs"]
mod tests;
