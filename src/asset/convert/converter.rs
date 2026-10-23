//! What a converter is, and the table of them: one row per converted format.

use super::{blend, fbx, Dependency, Staging};
use crate::asset::ImportError;
use crate::core::paths;
use std::path::Path;

/// One source format the importer converts to glTF.
pub struct Converter {
    /// The source extension, lowercase.
    pub extension: &'static str,
    /// What does the conversion, recorded in the stamp.
    pub name: &'static str,
    /// Bump when the converter's output changes, so every cached file re-makes.
    pub version: u32,
    /// The program the conversion runs, by the version it reports (`5.0.1`),
    /// recorded in the stamp so a different one re-converts; `None` for a
    /// converter inside the engine. An `Err` (the program is missing) keeps a
    /// cache of the unchanged source, as a missing source does.
    pub tool: Option<fn() -> Result<String, ImportError>>,
    /// Read `source` and write plain files where `to` says: the glTF (binary or
    /// not) at [`Staging::glb`], and the files it references that the conversion
    /// makes (FBX's embedded textures) in [`Staging::textures`]. [`refresh`](super::refresh) swaps
    /// them into place whole, and removes what an earlier conversion left. Any
    /// failure is an `Err`; only what is left out on purpose goes in [`Converted`].
    ///
    /// A conversion re-made only because a file it looked at changed reuses the
    /// textures folder (its name digests the stamp, not the files), so a written
    /// file's name must hold the same bytes whatever is found on disk: name every
    /// file the source embeds before looking at the disk (`fbx::texture`, #1189).
    pub convert: fn(source: &Path, to: &Staging) -> Result<Converted, ImportError>,
}

/// What a conversion reports beside the files it wrote.
#[derive(Debug, Default, PartialEq)]
pub struct Converted {
    /// What it left out on purpose, each with its reason.
    pub skipped: Vec<Skip>,
    /// Every file beside the source it looked at, found or not: the cache
    /// re-converts when one reads differently (`record`, #1189).
    pub depends_on: Vec<Dependency>,
}

/// One part of a source a conversion left out on purpose.
#[derive(Clone, Debug, PartialEq)]
pub struct Skip {
    /// What was left out (`FBX texture "Skin"`).
    pub what: String,
    /// Why (`embeds a .tga, not an image format the importer reads`).
    pub why: String,
}

impl Skip {
    /// The skip as one line: what is warned of, and stored under the stamp.
    pub(super) fn line(&self) -> String {
        format!("skipped {}: {}", self.what, self.why).replace(['\n', '\r'], " ")
    }
}

/// Every converted format.
pub const CONVERTERS: &[Converter] = &[
    Converter {
        extension: "fbx",
        name: "ufbx",
        // 2: animation stacks become glTF animations (#1081).
        // 3: a rest pose splits like its keys, keeping a negative scale's signs (#1110).
        // 4: embedded textures are extracted beside the `.glb` (#1082).
        // 5: the textures folder is named by the conversion's digest (#1129).
        // 6: a texture that fails to extract fails the conversion (#1168).
        // 7: an embedded texture's name is reserved even when a file on disk is
        //    used instead, and the files looked at are recorded (#1189).
        // 8: an extracted texture's image is named by its identity, which a saved
        //    scene references (#1130).
        // 9: an extracted texture's extension follows its bytes, not its name (#1072).
        version: 9,
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

/// The converter for `path`'s extension, if it is a converted format.
pub fn converter_for(path: &Path) -> Option<&'static Converter> {
    let ext = paths::extension(path);
    CONVERTERS.iter().find(|c| c.extension == ext)
}
