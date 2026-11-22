//! Splits on the FIRST `::`: a sub-object name is taken verbatim from the model and may hold `::` (`hero.glb::Body::Mesh`), so the path may not. Refused at build time, never saved unparseable.

use super::{is_model_path, ImportError};

/// Only the first occurrence separates; a sub id may hold more.
pub const REF_SEPARATOR: &str = "::";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRef {
    pub path: String,
    pub sub_object: String,
}

impl AssetRef {
    /// The path must end in a model extension and hold no `::`, and neither part may be empty, so [`AssetRef::parse`] reads back what this builds.
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

    pub fn to_string_ref(&self) -> String {
        format!("{}{}{}", self.path, REF_SEPARATOR, self.sub_object)
    }

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

pub(super) fn bad(reference: &str, why: &str) -> ImportError {
    ImportError::BadReference(format!("'{reference}': {why}"))
}
