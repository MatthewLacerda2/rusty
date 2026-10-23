//! [`MapRef`]: a texture map as a component stores it (#1256).

use std::borrow::Borrow;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A texture map as a component names it: a plain path, or a reference to a
/// texture an import extracted (`soldier.fbx#tex/Skin`, see the module docs of
/// [`super`]). It is deliberately **not a path**: it has no `AsRef<Path>` and no
/// `Deref<Target = str>`, so `image::open(map)` does not compile, and the way to
/// its file is [`MapRef::file`], which resolves a reference through the import
/// cache. [`MapRef::as_str`] is the spelling for keys, display and scripts.
///
/// Saved as the bare string, so a scene file reads the same as before.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MapRef(String);

impl MapRef {
    /// The map `spelling` names.
    pub fn new(spelling: impl Into<String>) -> Self {
        Self(spelling.into())
    }

    /// The map as stored: a cache key, a label, what the editor shows and a Lua
    /// script reads back. Never a file to open: [`Self::file`] is that, and every
    /// reader that opens one (`render::gpu::textures::open`, the UI's size
    /// reader, the lightmap bake) takes the `MapRef` itself (#1269).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The file this map reads from: a reference resolved through its source's
    /// import cache, a plain path as it is. `None` when a reference names no
    /// texture the cache holds.
    pub fn file(&self) -> Option<PathBuf> {
        super::file(&self.0)
    }

    /// The source a reference names (`soldier.fbx`), `None` for a plain path.
    pub fn source(&self) -> Option<&str> {
        super::source(&self.0)
    }

    /// Whether the map names nothing (an editor field left blank).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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

/// A table keyed by maps looks one up by its spelling.
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
