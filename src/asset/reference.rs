//! src/asset/reference.rs — the `path::sub_object` scene reference (#74, #1295).
//!
//! The one builder and the one parser of the string an `"Asset"` mesh rehydrates
//! from. The reference splits on its FIRST `::`: a sub-object name is taken verbatim
//! from the model and may hold `::` (`hero.glb::Body::Mesh`), so the path may not.
//! That is rusty's rule, not the OS's — macOS and Linux both allow `:` in names —
//! and it costs nothing in practice because references are project-relative: a
//! project folder named `my::game` never appears in them. A path that breaks it is
//! refused with a reason at build time, never saved as a reference that won't parse.

use super::{is_model_path, ImportError};

/// The separator between a source path and a sub-object id in a scene reference:
/// `crates.glb::Barrel`. Only its first occurrence separates: a sub id may hold
/// more (`crates.glb::Body::Mesh`).
pub const REF_SEPARATOR: &str = "::";

/// A scene reference: a source file path and the sub-object within it. This is the
/// path-based identity an `"Asset"` mesh reference rehydrates from. Build one with
/// [`AssetRef::new`] or [`AssetRef::parse`], which hold the same rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRef {
    pub path: String,
    pub sub_object: String,
}

impl AssetRef {
    /// A checked reference to `sub_object` in the model at `path`: the path must end
    /// in a [`MODEL_EXTENSIONS`](super::MODEL_EXTENSIONS) extension and hold no `::`, and neither part may be
    /// empty, so what this builds [`AssetRef::parse`] reads back.
    pub fn new(path: &str, sub_object: &str) -> Result<AssetRef, ImportError> {
        let reference = AssetRef {
            path: path.to_string(),
            sub_object: sub_object.to_string(),
        };
        let why = if path.contains(REF_SEPARATOR) {
            Some(PATH_HOLDS_SEPARATOR)
        } else {
            reference.shape_error()
        };
        match why {
            Some(why) => Err(bad(&reference.to_string_ref(), why)),
            None => Ok(reference),
        }
    }

    /// The canonical reference string `path::sub_object`.
    pub fn to_string_ref(&self) -> String {
        format!("{}{}{}", self.path, REF_SEPARATOR, self.sub_object)
    }

    /// Parse `path::sub_object`. The path ends at the first `::`, which must follow
    /// a model extension; everything after it is the sub id, `::` included.
    pub fn parse(reference: &str) -> Result<AssetRef, ImportError> {
        let (path, sub) = reference
            .split_once(REF_SEPARATOR)
            .ok_or_else(|| bad(reference, EXPECTED_SHAPE))?;
        let parsed = AssetRef {
            path: path.to_string(),
            sub_object: sub.to_string(),
        };
        let path_holds_separator = || {
            reference
                .match_indices(REF_SEPARATOR)
                .any(|(at, _)| is_model_path(&reference[..at]))
        };
        match parsed.shape_error() {
            Some(NOT_A_MODEL) if path_holds_separator() => {
                Err(bad(reference, PATH_HOLDS_SEPARATOR))
            }
            Some(why) => Err(bad(reference, why)),
            None => Ok(parsed),
        }
    }

    /// Why this isn't a well-formed reference, past the separator rule.
    fn shape_error(&self) -> Option<&'static str> {
        if self.path.is_empty() || self.sub_object.is_empty() {
            Some(EXPECTED_SHAPE)
        } else if !is_model_path(&self.path) {
            Some(NOT_A_MODEL)
        } else {
            None
        }
    }
}

const EXPECTED_SHAPE: &str = "expected `path::sub_object`";
const PATH_HOLDS_SEPARATOR: &str =
    "a file path may not contain `::` (a sub-object name may; references are project-relative)";
const NOT_A_MODEL: &str = "the file path must end in a model extension before the first `::`";

/// A [`ImportError::BadReference`] in its one shape: `'<reference>': <why>`.
pub(super) fn bad(reference: &str, why: &str) -> ImportError {
    ImportError::BadReference(format!("'{reference}': {why}"))
}
