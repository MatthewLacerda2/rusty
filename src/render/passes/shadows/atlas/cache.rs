//! Which atlas tiles still hold their static casters (#694), the atlas's version of
//! the cascades' `static_cache` (#355). Pure CPU.
//!
//! The same list keeps which tiles of the *active* atlas hold nothing but their
//! static copy (#1711): such a tile is left as it is while no dynamic caster
//! reaches it.
//!
//! A tile is keyed by its scene's [`StaticsKey`] and the whole [`Tile`] — view-projection, origin and
//! size — so it re-bakes exactly when its light moved or turned, the plan gave it
//! another place or size, another scene drew over it, or the scene's statics changed,
//! and never otherwise.
//! Baked tiles never overlap: baking one forgets every entry it drew over, so an
//! entry still listed is still in the static atlas, even when its light skipped a
//! frame (left the view, or lost its room) and came back.

use super::Tile;
use crate::render::passes::shadows::StaticsKey;

/// The tiles whose static casters the static atlas holds.
#[derive(Debug, Default)]
pub(super) struct StaticTiles {
    baked: Vec<(StaticsKey, Tile)>,
}

impl StaticTiles {
    /// Indices of `tiles` whose statics for `scene` are not in the static atlas.
    pub(super) fn stale(&self, scene: StaticsKey, tiles: &[Tile]) -> Vec<usize> {
        (0..tiles.len())
            .filter(|&i| !self.holds(scene, &tiles[i]))
            .collect()
    }

    /// Whether `tile`'s statics for `scene` are held.
    pub(super) fn holds(&self, scene: StaticsKey, tile: &Tile) -> bool {
        self.baked.contains(&(scene, *tile))
    }

    /// Record that `scene`'s `rebaked` tiles were baked, forgetting what they drew over.
    pub(super) fn commit(&mut self, scene: StaticsKey, rebaked: impl IntoIterator<Item = Tile>) {
        for tile in rebaked {
            self.forget(&tile);
            self.baked.push((scene, tile));
        }
    }

    /// Forget whatever `tile` drew over.
    pub(super) fn forget(&mut self, tile: &Tile) {
        self.baked.retain(|(_, old)| !overlaps(old, tile));
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
