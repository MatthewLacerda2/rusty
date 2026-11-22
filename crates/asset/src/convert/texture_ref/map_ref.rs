//! [`MapRef`]: a texture map as a component stores it.

use std::borrow::Borrow;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A texture map: a plain path, or a reference to an import-extracted texture
/// (`soldier.fbx#tex/Skin`). Deliberately not a path (no `AsRef<Path>` or
/// `Deref<Target = str>`), so `image::open(map)` does not compile; open it through
/// [`MapRef::file`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MapRef(String);

impl MapRef {
    pub fn new(spelling: impl Into<String>) -> Self {
        Self(spelling.into())
    }

    /// The stored spelling: cache key, label, what scripts read. Never a file to open.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `None` when a reference names no texture the cache holds.
    pub fn file(&self) -> Option<PathBuf> {
        super::file(&self.0)
    }

    /// Test hook, compiled in every build: the root's `ui` tests read it.
    #[doc(hidden)]
    pub fn source(&self) -> Option<&str> {
        super::source(&self.0)
    }
}

impl From<String> for MapRef {
    fn from(spelling: String) -> Self {
        Self(spelling)
    }
}

impl From<&str> for MapRef {
    fn from(spelling: &str) -> Self {
        Self(spelling.to_string())
    }
}

impl Borrow<str> for MapRef {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for MapRef {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for MapRef {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl fmt::Debug for MapRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for MapRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
