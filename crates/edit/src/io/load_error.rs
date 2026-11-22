use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    Document(String),
    /// The World was replaced, but its probes stay unbaked until the next Generate Lighting.
    Sidecar(String),
}

impl LoadError {
    /// When true the World is the new file's: the caller makes that its current scene file,
    /// or the next Save writes the new scene over the old file.
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
