//! The overlap guard charting grows against (#831): the 2D triangles a chart holds,
//! bucketed on a uniform grid so a candidate is tested only against its near
//! neighbours. A triangle spanning more than [`MAX_CELLS`] cells goes on a short
//! list every query checks instead, so one huge floor never floods the grid.
//!
//! Two triangles *overlap* when their interiors do; sharing an edge or a corner is
//! not overlap, which is how a chart's triangles meet.

use glam::Vec2;

use crate::core::collections::Map;

/// Cells one triangle may occupy before it goes on the always-checked list.
const MAX_CELLS: i64 = 64;

/// A chart's flattened triangles, bucketed by grid cell.
pub(super) struct OverlapGrid {
    cell: f32,
    triangles: Vec<[Vec2; 3]>,
    cells: Map<(i32, i32), Vec<u32>>,
    large: Vec<u32>,
}

impl OverlapGrid {
    pub fn new(cell: f32) -> Self {
        Self {
            cell,
            triangles: Vec::new(),
            cells: Map::default(),
            large: Vec::new(),
        }
    }

    /// Add a triangle. One with no area is never stored: it covers nothing.
    pub fn insert(&mut self, tri: [Vec2; 3]) {
        if area(&tri) <= 0.0 {
            return;
        }
        let id = self.triangles.len() as u32;
        self.triangles.push(tri);
        match self.range(&tri) {
            Some((lo, hi)) => {
                for y in lo.1..=hi.1 {
                    for x in lo.0..=hi.0 {
                        self.cells.entry((x, y)).or_default().push(id);
                    }
                }
            }
            None => self.large.push(id),
        }
    }

    /// Whether `tri`'s interior overlaps a stored triangle's. A triangle with no area
    /// overlaps nothing.
    pub fn overlaps(&self, tri: &[Vec2; 3]) -> bool {
        if area(tri) <= 0.0 {
            return false;
        }
        let eps = self.cell * 1e-5;
        let hit = |id: &u32| interiors_overlap(tri, &self.triangles[*id as usize], eps);
        if self.large.iter().any(hit) {
            return true;
        }
        let Some((lo, hi)) = self.range(tri) else {
            // Too large to bucket: test it against everything.
            return (0..self.triangles.len() as u32).any(|id| hit(&id));
        };
        (lo.1..=hi.1).any(|y| {
            (lo.0..=hi.0).any(|x| {
                self.cells
                    .get(&(x, y))
                    .is_some_and(|ids| ids.iter().any(hit))
            })
        })
    }

    /// The inclusive cell range `tri`'s bounds cover, or `None` past [`MAX_CELLS`].
    fn range(&self, tri: &[Vec2; 3]) -> Option<((i32, i32), (i32, i32))> {
        let lo = tri[0].min(tri[1]).min(tri[2]) / self.cell;
        let hi = tri[0].max(tri[1]).max(tri[2]) / self.cell;
        let (lo, hi) = (lo.floor(), hi.floor());
        let span = (hi.x - lo.x + 1.0) as i64 * (hi.y - lo.y + 1.0) as i64;
        let finite = lo.is_finite() && hi.is_finite();
        let fits = lo.abs().max_element().max(hi.abs().max_element()) < i32::MAX as f32;
        (finite && fits && span <= MAX_CELLS)
            .then(|| ((lo.x as i32, lo.y as i32), (hi.x as i32, hi.y as i32)))
    }
}

/// Twice the unsigned area of `tri`.
fn area(tri: &[Vec2; 3]) -> f32 {
    (tri[1] - tri[0]).perp_dot(tri[2] - tri[0]).abs()
}

/// Separating-axis test on the six edge normals: the interiors overlap unless some
/// axis separates the two, touching (within `eps`) counting as separated.
pub(super) fn interiors_overlap(a: &[Vec2; 3], b: &[Vec2; 3], eps: f32) -> bool {
    let edges = (0..3).flat_map(|k| [a[(k + 1) % 3] - a[k], b[(k + 1) % 3] - b[k]]);
    for edge in edges {
        let axis = edge.perp();
        if axis == Vec2::ZERO {
            continue;
        }
        let axis = axis.normalize();
        let (a_lo, a_hi) = extent(a, axis);
        let (b_lo, b_hi) = extent(b, axis);
        if a_hi <= b_lo + eps || b_hi <= a_lo + eps {
            return false;
        }
    }
    true
}

fn extent(tri: &[Vec2; 3], axis: Vec2) -> (f32, f32) {
    let d = tri.map(|p| p.dot(axis));
    (d[0].min(d[1]).min(d[2]), d[0].max(d[1]).max(d[2]))
}
