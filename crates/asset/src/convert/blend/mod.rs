//! Blender runs headless with export settings pinned in `export.py` (+Y up, modifiers applied,
//! animations and skins on), so two machines with the same Blender make the same glTF.
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod drop;
mod known;
mod locate;
mod pack;
mod run;

pub use locate::Search;

use super::stage::hook::{self, Step};
use super::{Converted, Staging};
use crate::ImportError;
use std::path::Path;

const EXPORT_DIR: &str = "blender";

pub fn convert(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    convert_with(&Search::from_env(), source, to)
}

pub fn identify() -> Result<String, ImportError> {
    Search::from_env().find().map(|blender| blender.version)
}

fn convert_with(search: &Search, source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    let blender = search.find()?;
    let staging = to.glb.parent().unwrap_or_else(|| Path::new("."));
    // Absolute: Blender writes the image URIs relative to it, which `pack` joins.
    let dir = std::path::absolute(staging.join(EXPORT_DIR))
        .map_err(|e| ImportError::Parse(format!("{}: {e}", staging.display())))?;
    hook::io(Step::ReadSource)
        .and_then(|()| std::fs::create_dir(&dir))
        .map_err(|e| ImportError::Parse(format!("writing {}: {e}", dir.display())))?;
    run::export(&blender.path, source, &dir, search.export_timeout()?)?;
    pack::pack(&dir, to)
}

#[cfg(test)]
mod tests;
