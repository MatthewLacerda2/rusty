use std::path::Path;

use super::engine_shader_dir;
use super::params::{sidecar_path, ParamLayout};
use super::rebake::{self, Plan};

#[derive(Debug, Clone, PartialEq)]
pub struct Baked {
    pub source: String,
    pub params: Option<String>,
}

impl Baked {
    pub fn layout(&self, wgsl: &Path) -> Result<ParamLayout, String> {
        let Some(json) = &self.params else {
            return Ok(ParamLayout::default());
        };
        serde_json::from_str(json).map_err(|e| format!("{}: {e}", sidecar_path(wgsl).display()))
    }
}

pub fn load(wgsl: &Path) -> Result<Baked, String> {
    load_against(wgsl, engine_shader_dir(), rebake::WRITES)
}

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
