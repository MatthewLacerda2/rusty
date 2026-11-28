//! A bounds change, and finding the cell an id names.

use super::tiles::{Tile, TileGrid, TileKey};
use super::NavSpan;
use crate::bake::CellRect;

impl TileGrid {
    /// The cell holding each id: `(gx, gz, offset in the column)`.
    pub fn locator(&self) -> impl Fn(u32) -> Option<(i32, i32, u32)> + '_ {
        let mut by_base: Vec<&Tile> = self.tiles.iter().flatten().collect();
        by_base.sort_unstable_by_key(|t| t.base);
        move |id: u32| {
            let t = by_base[by_base.partition_point(|t| t.base <= id).checked_sub(1)?];
            let off = id - t.base;
            if off >= t.len() {
                return None;
            }
            let l = t.cell_start.partition_point(|&s| s <= off) - 1;
            let s = self.side;
            let (lx, lz) = (l as i32 % s, l as i32 / s);
            let gx = t.key.0 * s + lx - self.origin.0;
            let gz = t.key.1 * s + lz - self.origin.1;
            Some((gx, gz, off - t.cell_start[l]))
        }
    }
    /// A tile the new bounds leave is dropped and one they cut is clipped; the rest keep their ids.
    pub fn reframe(
        &mut self,
        origin: (i32, i32),
        width: i32,
        height: i32,
        store: &mut Vec<NavSpan>,
    ) {
        let old = std::mem::replace(self, Self::new(origin, width, height, self.side));
        self.dead = old.dead;
        for t in old.tiles.into_iter().flatten() {
            let Some(rect) = self.rect(t.key) else {
                self.dead += t.len();
                continue;
            };
            let slot = self.claim(t.key);
            let key = t.key;
            self.tiles[slot] = Some(t);
            self.clip(key, rect, store);
        }
    }

    fn clip(&mut self, key: TileKey, rect: CellRect, store: &mut Vec<NavSpan>) {
        let t = self.tile(key).expect("a stored tile");
        let s = self.side;
        let (x0, z0) = (key.0 * s - self.origin.0, key.1 * s - self.origin.1);
        let inside = |l: usize| rect.contains(x0 + l as i32 % s, z0 + l as i32 / s);
        let cells = (s * s) as usize;
        let cut = (0..cells).any(|l| !inside(l) && t.cell_start[l] != t.cell_start[l + 1]);
        if !cut {
            return;
        }
        let mut starts = vec![0];
        let mut spans = Vec::new();
        for l in 0..cells {
            if inside(l) {
                let r = t.cell_start[l] as usize..t.cell_start[l + 1] as usize;
                let base = t.base as usize;
                spans.extend_from_slice(&store[base + r.start..base + r.end]);
            }
            starts.push(spans.len() as u32);
        }
        self.write(key, starts, &spans, store);
    }
}
