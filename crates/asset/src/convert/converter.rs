use super::{blend, fbx, Dependency, Staging};
use crate::ImportError;
use rusty_core::paths;
use std::path::Path;

pub struct Converter {
    pub extension: &'static str,
    pub name: &'static str,
    /// Bump when the output changes, so every cache re-makes (`fingerprint` holds it).
    pub version: u32,
    /// The program's reported version (`5.0.1`), stamped so another re-converts;
    /// `None` inside the engine. An `Err` (missing) keeps the cache.
    pub tool: Option<fn() -> Result<String, ImportError>>,
    /// Never logs: only what is left out on purpose goes in [`Converted`], any other
    /// failure is an `Err`, or a partial cache is stamped current. A conversion whose
    /// stamp was lost meets its own textures folder again (`record::folder_tag`), so a
    /// written file's name must hold the same bytes whatever is on disk: name every
    /// embedded file first.
    pub convert: fn(source: &Path, to: &Staging) -> Result<Converted, ImportError>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Converted {
    pub skipped: Vec<Skip>,
    /// Every file beside the source it looked at, found or not.
    pub depends_on: Vec<Dependency>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Skip {
    pub what: String,
    pub why: String,
}

impl Skip {
    pub(super) fn line(&self) -> String {
        format!("skipped {}: {}", self.what, self.why).replace(['\n', '\r'], " ")
    }
}

pub const CONVERTERS: &[Converter] = &[
    Converter {
        extension: "fbx",
        name: "ufbx",
        version: 13,
        tool: None,
        convert: fbx::convert,
    },
    Converter {
        extension: "blend",
        name: "blender",
        version: 1,
        tool: Some(blend::identify),
        convert: blend::convert,
    },
];

pub fn converter_for(path: &Path) -> Option<&'static Converter> {
    let ext = paths::extension(path);
    CONVERTERS.iter().find(|c| c.extension == ext)
}
