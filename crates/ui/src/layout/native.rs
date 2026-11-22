use std::collections::BTreeMap;

use glam::Vec2;

#[derive(Clone, Debug, Default)]
pub struct TextureSizes {
    sizes: BTreeMap<String, Vec2>,
    /// Writes so far ([`Self::edits`]).
    edits: u64,
}

/// Two tables are equal when they hold the same sizes, however they got there.
impl PartialEq for TextureSizes {
    fn eq(&self, other: &Self) -> bool {
        self.sizes == other.sizes
    }
}

impl TextureSizes {
    /// The size of the texture at `path`, when the platform knows it.
    pub fn get(&self, path: &str) -> Option<Vec2> {
        self.sizes.get(path).copied()
    }

    /// Record `path`'s size in texels.
    pub fn set(&mut self, path: impl Into<String>, size: Vec2) {
        self.sizes.insert(path.into(), size);
        self.edits += 1;
    }

    /// Forget `path`'s size.
    pub fn remove(&mut self, path: &str) {
        self.edits += u64::from(self.sizes.remove(path).is_some());
    }

    /// How many writes changed the table: a filler that remembers it knows when
    /// someone else edited the table since.
    pub fn edits(&self) -> u64 {
        self.edits
    }

    /// Every path the table holds a size for.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.sizes.keys().map(String::as_str)
    }
}
