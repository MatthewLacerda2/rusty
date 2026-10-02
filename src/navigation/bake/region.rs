//! src/navigation/bake/region.rs — a rectangle of grid cells, the unit of an
//! incremental rebake (#456).
//!
//! The bake picks **dirty rectangles** over fixed tiles: a moved crate dirties
//! exactly the cells its old and new footprints cover, so the work tracks the size
//! of the change rather than a tile size, and there are no tile seams to stitch.

use glam::Vec3;

use super::super::NavigationGraph;

/// An inclusive rectangle of cells, `x0..=x1` × `z0..=z1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRect {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
}

impl CellRect {
    /// Every cell of `g`.
    pub fn all(g: &NavigationGraph) -> Self {
        Self {
            x0: 0,
            z0: 0,
            x1: g.width - 1,
            z1: g.height - 1,
        }
    }

    /// The cells whose squares an XZ box `lo..hi` reaches into, clamped to `g`;
    /// `None` when the box lies off the grid.
    pub fn covering(g: &NavigationGraph, lo: Vec3, hi: Vec3) -> Option<Self> {
        let s = g.grid_spacing;
        let cell_of = |v: f32, origin: f32| ((v - origin) / s).round() as i32;
        Self {
            x0: cell_of(lo.x, g.min_x),
            z0: cell_of(lo.z, g.min_z),
            x1: cell_of(hi.x, g.min_x),
            z1: cell_of(hi.z, g.min_z),
        }
        .clamped(g)
    }

    /// Grown by `n` cells on every side (not clamped).
    pub fn grown(self, n: i32) -> Self {
        Self {
            x0: self.x0 - n,
            z0: self.z0 - n,
            x1: self.x1 + n,
            z1: self.z1 + n,
        }
    }

    /// Clipped to `g`'s cells; `None` when nothing is left.
    pub fn clamped(self, g: &NavigationGraph) -> Option<Self> {
        self.intersection(Self::all(g))
    }

    /// The cells both rectangles hold, `None` when they are disjoint.
    pub fn intersection(self, o: Self) -> Option<Self> {
        let r = Self {
            x0: self.x0.max(o.x0),
            z0: self.z0.max(o.z0),
            x1: self.x1.min(o.x1),
            z1: self.z1.min(o.z1),
        };
        (r.x0 <= r.x1 && r.z0 <= r.z1).then_some(r)
    }

    /// The smallest rectangle holding both.
    pub fn union(self, o: Self) -> Self {
        Self {
            x0: self.x0.min(o.x0),
            z0: self.z0.min(o.z0),
            x1: self.x1.max(o.x1),
            z1: self.z1.max(o.z1),
        }
    }

    pub fn contains(self, gx: i32, gz: i32) -> bool {
        gx >= self.x0 && gx <= self.x1 && gz >= self.z0 && gz <= self.z1
    }

    pub fn width(self) -> i32 {
        self.x1 - self.x0 + 1
    }

    /// Number of cells.
    pub fn cells(self) -> usize {
        (self.width() * (self.z1 - self.z0 + 1)) as usize
    }

    /// The world XZ box the cells' squares cover (`y` is zero).
    pub fn world_box(self, g: &NavigationGraph) -> (Vec3, Vec3) {
        let half = g.grid_spacing * 0.5;
        let lo = g.cell_center(self.x0, self.z0) - Vec3::new(half, 0.0, half);
        let hi = g.cell_center(self.x1, self.z1) + Vec3::new(half, 0.0, half);
        (lo, hi)
    }
}

/// Merge rectangles until none overlap, in a deterministic order: a pair is
/// replaced by its union, so the result covers every input cell.
pub fn merge_overlapping(mut rects: Vec<CellRect>) -> Vec<CellRect> {
    let mut merged = true;
    while merged {
        merged = false;
        'scan: for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                if rects[i].intersection(rects[j]).is_some() {
                    let other = rects.swap_remove(j);
                    rects[i] = rects[i].union(other);
                    merged = true;
                    break 'scan;
                }
            }
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: i32, z0: i32, x1: i32, z1: i32) -> CellRect {
        CellRect { x0, z0, x1, z1 }
    }

    #[test]
    fn covering_maps_a_box_to_its_cells_and_clamps() {
        let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        let c = CellRect::covering(&g, Vec3::new(2.2, 0.0, -5.0), Vec3::new(3.7, 0.0, 1.4));
        assert_eq!(c, Some(r(2, 0, 4, 1)));
        let off = CellRect::covering(&g, Vec3::splat(20.0), Vec3::splat(30.0));
        assert_eq!(off, None, "a box past the grid covers nothing");
    }

    #[test]
    fn merging_joins_overlaps_and_keeps_disjoint_rects() {
        let rects = vec![r(0, 0, 2, 2), r(8, 8, 9, 9), r(2, 2, 4, 4)];
        let mut out = merge_overlapping(rects);
        out.sort_by_key(|c| (c.x0, c.z0));
        assert_eq!(out, vec![r(0, 0, 4, 4), r(8, 8, 9, 9)]);
    }

    #[test]
    fn world_box_spans_the_cell_squares() {
        let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        let (lo, hi) = r(1, 2, 3, 2).world_box(&g);
        assert_eq!((lo.x, lo.z, hi.x, hi.z), (0.5, 1.5, 3.5, 2.5));
        assert_eq!(r(1, 2, 3, 2).cells(), 3);
    }
}
