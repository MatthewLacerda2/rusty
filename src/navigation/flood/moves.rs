//! The flood's step: A\*'s moves and costs out of one span, and the heap entry
//! that orders the flood.

use crate::navigation::{in_mask, NavSpan, NavigationGraph, SpanRef, MIN_AREA_COST};

/// A span a move reaches, with the cost it reaches it at.
pub(super) type Move = (SpanRef, f32);

/// A heap entry: a span, the cost it was reached at, and its key (that cost
/// plus the heuristic, `goals`), packed so the heap compares one integer. A key
/// is never negative, so its bits order as the key does; the span index below
/// them breaks ties, as A\* breaks them.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) struct Open {
    key: u64,
    g: u32,
    gx: i32,
    gz: i32,
}

impl Open {
    pub(super) fn new(node: SpanRef, g: f32, f: f32) -> Self {
        debug_assert!(g >= 0.0 && f >= 0.0, "a flood cost is never negative");
        Self {
            key: (u64::from(f.to_bits()) << 32) | u64::from(node.index),
            g: g.to_bits(),
            gx: node.gx,
            gz: node.gz,
        }
    }

    pub(super) fn node(self) -> SpanRef {
        SpanRef {
            gx: self.gx,
            gz: self.gz,
            index: self.key as u32,
        }
    }

    pub(super) fn g(self) -> f32 {
        f32::from_bits(self.g)
    }

    /// The same entry for `node`, its span renumbered by a rebake (`carry`):
    /// its key and cost are kept, only the tie-breaking index moves.
    pub(super) fn moved(self, node: SpanRef) -> Self {
        Self {
            key: (self.key & !u64::from(u32::MAX)) | u64::from(node.index),
            gx: node.gx,
            gz: node.gz,
            ..self
        }
    }
}

impl Ord for Open {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Min-heap on the key; ties on the span index, so the pop order is fixed.
        other.key.cmp(&self.key)
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The 8-way moves: (dx, dz, horizontal cost in cells), as A\* takes them.
pub(super) const DIRS: [(i32, i32, f32); 8] = [
    (1, 0, 1.0),
    (-1, 0, 1.0),
    (0, 1, 1.0),
    (0, -1, 1.0),
    (1, 1, std::f32::consts::SQRT_2),
    (1, -1, std::f32::consts::SQRT_2),
    (-1, 1, std::f32::consts::SQRT_2),
    (-1, -1, std::f32::consts::SQRT_2),
];

impl NavigationGraph {
    /// Every move out of `here` with the cost it reaches its span at, appended to
    /// `out`: A\*'s `expand`, step for step. `links` says whether an off-mesh
    /// link may leave `here`; without one, its links are not looked up.
    pub(super) fn moves(&self, here: Open, mask: u32, links: bool, out: &mut Vec<Move>) {
        let (at, g) = (here.node(), here.g());
        let NavSpan { y, area, .. } = self.spans[at.index as usize];
        let step = |dx: i32, dz: i32| {
            let to = self.link_to(at, at.gx + dx, at.gz + dz)?;
            in_mask(mask, self.spans[to.index as usize].area).then_some(to)
        };
        let (east, west, north, south) = (step(1, 0), step(-1, 0), step(0, 1), step(0, -1));
        let cost_of = |s: SpanRef| self.area_cost(self.spans[s.index as usize].area);
        for &(dx, dz, horiz) in &DIRS {
            let (next, corner) = match (dx, dz) {
                (1, 0) => (east, MIN_AREA_COST),
                (-1, 0) => (west, MIN_AREA_COST),
                (0, 1) => (north, MIN_AREA_COST),
                (0, -1) => (south, MIN_AREA_COST),
                _ => {
                    let x = if dx > 0 { east } else { west };
                    let z = if dz > 0 { north } else { south };
                    let (Some(x), Some(z)) = (x, z) else {
                        continue;
                    };
                    (step(dx, dz), cost_of(x).max(cost_of(z)))
                }
            };
            let Some(next) = next else {
                continue;
            };
            let to = self.spans[next.index as usize];
            let cost = (0.5 * (self.area_cost(area) + cost_of(next))).max(corner);
            out.push((next, g + (horiz + (to.y - y).abs()) * cost));
        }
        if !links {
            return;
        }
        for link in self.link_moves(at) {
            let lands = self.spans[link.to.index as usize].area;
            if in_mask(mask, link.area) && in_mask(mask, lands) {
                out.push((link.to, g + link.cost));
            }
        }
    }
}
