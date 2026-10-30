//! Holding the decoded probe cubes to the budget (#1584).
//!
//! The renderer keeps each probe cube it loads in its by-path cache (#355), which
//! the array copies from. Left alone, a walk through every room of a level would
//! keep every cube of the level there. This table remembers, for every cube the
//! cache holds, its size and when it was last resident, and after each frame names
//! the least recently resident to evict until the cache's bytes are back within
//! the scene's budget. The cubes resident this frame are never evicted, so the
//! cache can pass the budget only when the nearest cube alone does.
//!
//! It also remembers the size of every cube it has seen, evicted or not, so the
//! frame can tell that a far cube would not fit the budget without reading its
//! file again.

use std::collections::HashMap;

use super::frame::array_bytes;

/// The probe cubes the renderer's cache holds, least recently resident first, and
/// the size of every cube ever loaded.
#[derive(Debug, Default)]
pub(crate) struct CubeCache {
    /// Paths of the cubes the cache holds, least recently resident first.
    held: Vec<String>,
    /// The face size of each cube loaded, kept after it is evicted.
    sizes: HashMap<String, u32>,
}

impl CubeCache {
    /// The face size of the cube at `path`, if it was ever loaded.
    pub(crate) fn size(&self, path: &str) -> Option<u32> {
        self.sizes.get(path).copied()
    }

    /// Record that the cache now holds the `size`-texel cube at `path`.
    pub(crate) fn loaded(&mut self, path: &str, size: u32) {
        self.sizes.insert(path.to_string(), size);
        if !self.held.iter().any(|p| p == path) {
            self.held.push(path.to_string());
        }
    }

    /// Mark `resident` as the most recently resident, in order, and return the
    /// paths to evict, least recently resident first, until the cubes held take at
    /// most `budget` bytes. A resident cube is never evicted.
    pub(crate) fn evict(&mut self, resident: &[&str], budget: u64) -> Vec<String> {
        for path in resident {
            if let Some(i) = self.held.iter().position(|p| p == path) {
                let p = self.held.remove(i);
                self.held.push(p);
            }
        }
        let bytes = |p: &String| self.sizes.get(p).map_or(0, |&s| array_bytes(s, 1));
        let mut total: u64 = self.held.iter().map(bytes).sum();
        let mut evicted = Vec::new();
        for path in &self.held {
            if total <= budget {
                break;
            }
            if resident.contains(&path.as_str()) {
                continue;
            }
            total -= bytes(path);
            evicted.push(path.clone());
        }
        self.held.retain(|p| !evicted.contains(p));
        evicted
    }

    /// Forget every cube `stale` names (a re-written file, #1207): its size may
    /// have changed, and the cache drops it.
    pub(crate) fn forget(&mut self, stale: impl Fn(&str) -> bool) {
        self.held.retain(|p| !stale(p));
        self.sizes.retain(|p, _| !stale(p));
    }

    /// The bytes of the cubes the cache holds.
    #[cfg(test)]
    pub(crate) fn held_bytes(&self) -> u64 {
        let bytes = |p: &String| self.sizes.get(p).map_or(0, |&s| array_bytes(s, 1));
        self.held.iter().map(bytes).sum()
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod cache_tests;
