use super::Tile;
use crate::passes::shadows::StaticsKey;

#[derive(Debug, Default)]
pub(super) struct StaticTiles {
    baked: Vec<(StaticsKey, Tile)>,
}

impl StaticTiles {
    pub(super) fn stale(&self, scene: StaticsKey, tiles: &[Tile]) -> Vec<usize> {
        (0..tiles.len())
            .filter(|&i| !self.holds(scene, &tiles[i]))
            .collect()
    }

    pub(super) fn holds(&self, scene: StaticsKey, tile: &Tile) -> bool {
        self.baked.contains(&(scene, *tile))
    }

    pub(super) fn commit(&mut self, scene: StaticsKey, rebaked: impl IntoIterator<Item = Tile>) {
        for tile in rebaked {
            self.forget(&tile);
            self.baked.push((scene, tile));
        }
    }

    pub(super) fn forget(&mut self, tile: &Tile) {
        self.baked.retain(|(_, old)| !overlaps(old, tile));
    }

    pub(super) fn clear(&mut self) {
        self.baked.clear();
    }
}

fn overlaps(a: &Tile, b: &Tile) -> bool {
    let apart = |a: &Tile, b: &Tile, axis: usize| a.origin[axis] + a.size <= b.origin[axis];
    !(0..2).any(|axis| apart(a, b, axis) || apart(b, a, axis))
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
