//! Which atlas tiles still hold their static casters (#694), the atlas's version of
//! the cascades' `static_cache` (#355). Pure CPU.
//!
//! A tile is keyed by its scene and the whole [`Tile`] — view-projection, origin and
//! size — so it re-bakes exactly when its light moved or turned, the plan gave it
//! another place or size, or another scene drew over it, and never otherwise.
//! Baked tiles never overlap: baking one forgets every entry it drew over, so an
//! entry still listed is still in the static atlas, even when its light skipped a
//! frame (left the view, or lost its room) and came back.

use super::Tile;
use crate::scene::SceneId;

/// The tiles whose static casters the static atlas holds.
#[derive(Debug, Default)]
pub(super) struct StaticTiles {
    baked: Vec<(SceneId, Tile)>,
}

impl StaticTiles {
    /// Indices of `tiles` whose statics for `scene` are not in the static atlas.
    pub(super) fn stale(&self, scene: SceneId, tiles: &[Tile]) -> Vec<usize> {
        (0..tiles.len())
            .filter(|&i| !self.baked.contains(&(scene, tiles[i])))
            .collect()
    }

    /// Record that `scene`'s `rebaked` tiles were baked, forgetting what they drew over.
    pub(super) fn commit(&mut self, scene: SceneId, rebaked: impl IntoIterator<Item = Tile>) {
        for tile in rebaked {
            self.baked.retain(|(_, old)| !overlaps(old, &tile));
            self.baked.push((scene, tile));
        }
    }

    /// Forget every bake, so each tile re-bakes on its next frame.
    pub(super) fn clear(&mut self) {
        self.baked.clear();
    }
}

/// Whether two tiles share any texel.
fn overlaps(a: &Tile, b: &Tile) -> bool {
    let apart = |a: &Tile, b: &Tile, axis: usize| a.origin[axis] + a.size <= b.origin[axis];
    !(0..2).any(|axis| apart(a, b, axis) || apart(b, a, axis))
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
