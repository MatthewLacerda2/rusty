//! Why a scene load failed, and whether it replaced the World first (#1374).

use std::fmt;

/// A failed [`load_from_file`](super::load_from_file). The two cases differ in what
/// the caller must do: after [`LoadError::Sidecar`] the World *is* the new file's, so
/// the editor's current scene file must become that file too, or the next Save
/// writes the new scene's entities over the old file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The document was not read or did not parse: the World is untouched.
    Document(String),
    /// The document loaded and replaced the World, but its lighting sidecar did
    /// not: the probes keep zero SH until the next bake.
    Sidecar(String),
}

impl LoadError {
    /// Whether the World was replaced before the load failed.
    pub fn replaced_world(&self) -> bool {
        matches!(self, Self::Sidecar(_))
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(err) => f.write_str(err),
            Self::Sidecar(err) => write!(f, "the scene loaded, but not its lighting: {err}"),
        }
    }
}

impl From<String> for LoadError {
    fn from(err: String) -> Self {
        Self::Document(err)
    }
}
