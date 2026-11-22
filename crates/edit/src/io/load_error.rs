use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    Document(String),
    Sidecar(String),
}

impl LoadError {
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
