//! Tiles of `side` columns, keyed on the world lattice (`(x, z).div_euclid(side)` in cells), so
//! a key never depends on the bounds. Only a tile holding a span exists. A span id is its tile's
//! `base` plus its index there: a tile outgrowing its range moves to the store's end, so no write
//! renumbers another tile. Ids left behind are `dead` until `compact`.

use std::ops::Range;

use super::NavSpan;
use crate::bake::CellRect;

pub type TileKey = (i32, i32);

const NONE: u32 = u32::MAX;

/// Unity's ceiling.
pub const MAX_TILE_SIDE: i32 = 1_024;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tile {
    pub key: TileKey,
    pub base: u32,
    cap: u32,
    pub cell_start: Vec<u32>,
}

impl Tile {
    pub fn len(&self) -> u32 {
        self.cell_start.last().copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
/// `origin`: the world column (in cells) of graph cell `(0, 0)`.
pub(crate) struct TileGrid {
    pub side: i32,
    pub origin: (i32, i32),
    pub width: i32,
    pub height: i32,
    first: TileKey,
    across: i32,
    lookup: Vec<u32>,
    pub(super) tiles: Vec<Option<Tile>>,
    free: Vec<u32>,
    pub dead: u32,
}

impl TileGrid {
    pub fn new(origin: (i32, i32), width: i32, height: i32, side: i32) -> Self {
        let side = side.max(1);
        let tile = |v: i32| v.div_euclid(side);
        let first = (tile(origin.0), tile(origin.1));
        let across = tile(origin.0 + width.max(1) - 1) - first.0 + 1;
        let down = tile(origin.1 + height.max(1) - 1) - first.1 + 1;
        Self {
            side,
            origin,
            width,
            height,
            first,
            across,
            lookup: vec![NONE; (across * down) as usize],
            tiles: Vec::new(),
            free: Vec::new(),
            dead: 0,
        }
    }

    pub fn emptied(&self) -> Self {
        Self::new(self.origin, self.width, self.height, self.side)
    }

    pub fn same_frame(&self, o: &Self) -> bool {
        (self.side, self.origin, self.width, self.height) == (o.side, o.origin, o.width, o.height)
    }

    pub fn key_of(&self, gx: i32, gz: i32) -> TileKey {
        let s = self.side;
        (
            (gx + self.origin.0).div_euclid(s),
            (gz + self.origin.1).div_euclid(s),
        )
    }

    fn slot_at(&self, key: TileKey) -> Option<usize> {
        let (tx, tz) = (key.0 - self.first.0, key.1 - self.first.1);
        if tx < 0 || tz < 0 || tx >= self.across {
            return None;
        }
        let slot = *self.lookup.get((tz * self.across + tx) as usize)?;
        (slot != NONE).then_some(slot as usize)
    }

    pub fn tile(&self, key: TileKey) -> Option<&Tile> {
        self.tiles[self.slot_at(key)?].as_ref()
    }

    /// Past the bounds too.
    pub fn span(&self, key: TileKey) -> CellRect {
        let x0 = key.0 * self.side - self.origin.0;
        let z0 = key.1 * self.side - self.origin.1;
        CellRect {
            x0,
            z0,
            x1: x0 + self.side - 1,
            z1: z0 + self.side - 1,
        }
    }

    pub fn rect(&self, key: TileKey) -> Option<CellRect> {
        let all = CellRect {
            x0: 0,
            z0: 0,
            x1: self.width - 1,
            z1: self.height - 1,
        };
        self.span(key).intersection(all)
    }

    pub fn keys_over(&self, r: CellRect) -> impl Iterator<Item = TileKey> {
        let (lo, hi) = (self.key_of(r.x0, r.z0), self.key_of(r.x1, r.z1));
        (lo.1..=hi.1).flat_map(move |tz| (lo.0..=hi.0).map(move |tx| (tx, tz)))
    }

    pub fn keys(&self) -> Vec<TileKey> {
        let mut keys: Vec<TileKey> = self.tiles.iter().flatten().map(|t| t.key).collect();
        keys.sort_unstable();
        keys
    }

    pub fn bytes(&self) -> usize {
        let starts: usize = self
            .tiles
            .iter()
            .flatten()
            .map(|t| t.cell_start.len() * 4)
            .sum();
        starts + self.lookup.len() * 4 + self.tiles.len() * std::mem::size_of::<Option<Tile>>()
    }

    pub fn count(&self) -> usize {
        self.tiles.iter().flatten().count()
    }

    pub fn local(&self, gx: i32, gz: i32) -> usize {
        let s = self.side;
        let (lx, lz) = (
            (gx + self.origin.0).rem_euclid(s),
            (gz + self.origin.1).rem_euclid(s),
        );
        (lz * s + lx) as usize
    }

    pub fn range(&self, gx: i32, gz: i32) -> Range<usize> {
        let Some(t) = self.tile(self.key_of(gx, gz)) else {
            return 0..0;
        };
        let l = self.local(gx, gz);
        let base = t.base as usize;
        base + t.cell_start[l] as usize..base + t.cell_start[l + 1] as usize
    }

    /// In place when the spans fit the tile's range; an empty tile is removed.
    pub fn write(
        &mut self,
        key: TileKey,
        starts: Vec<u32>,
        spans: &[NavSpan],
        store: &mut Vec<NavSpan>,
    ) {
        let len = spans.len() as u32;
        if len == 0 {
            self.remove(key, store);
            return;
        }
        let slot = match self.slot_at(key) {
            Some(slot) => slot,
            None => self.claim(key),
        };
        let t = self.tiles[slot].get_or_insert_with(|| Tile {
            key,
            base: store.len() as u32,
            cap: 0,
            cell_start: Vec::new(),
        });
        let old_len = t.len();
        let at_end = (t.base + t.cap) as usize == store.len();
        if len <= t.cap {
            self.dead = self.dead + old_len - len;
        } else if at_end {
            self.dead -= t.cap - old_len;
            store.truncate(t.base as usize);
            t.cap = len;
        } else {
            self.dead += old_len;
            t.base = store.len() as u32;
            t.cap = len;
        }
        let base = t.base as usize;
        if store.len() < base + len as usize {
            store.resize(base + len as usize, spans[0]);
        }
        store[base..base + len as usize].copy_from_slice(spans);
        t.cell_start = starts;
    }

    pub(super) fn claim(&mut self, key: TileKey) -> usize {
        let slot = self.free.pop().map_or(self.tiles.len(), |s| s as usize);
        if slot == self.tiles.len() {
            self.tiles.push(None);
        }
        let (tx, tz) = (key.0 - self.first.0, key.1 - self.first.1);
        self.lookup[(tz * self.across + tx) as usize] = slot as u32;
        slot
    }

    pub fn remove(&mut self, key: TileKey, store: &mut Vec<NavSpan>) -> Option<Tile> {
        let slot = self.slot_at(key)?;
        let t = self.tiles[slot].take()?;
        let (tx, tz) = (key.0 - self.first.0, key.1 - self.first.1);
        self.lookup[(tz * self.across + tx) as usize] = NONE;
        self.free.push(slot as u32);
        if (t.base + t.cap) as usize == store.len() {
            store.truncate(t.base as usize);
            self.dead -= t.cap - t.len();
        } else {
            self.dead += t.len();
        }
        Some(t)
    }

    pub fn wasteful(&self, store: &[NavSpan]) -> bool {
        self.dead as usize * 2 > store.len()
    }

    pub fn compact(&mut self, store: &mut Vec<NavSpan>) {
        let mut out = Vec::with_capacity(store.len() - self.dead as usize);
        for key in self.keys() {
            let slot = self.slot_at(key).expect("a stored key");
            let t = self.tiles[slot].as_mut().expect("a stored tile");
            let live = t.base as usize..(t.base + t.len()) as usize;
            t.base = out.len() as u32;
            t.cap = t.len();
            out.extend_from_slice(&store[live]);
        }
        *store = out;
        self.dead = 0;
    }
}
