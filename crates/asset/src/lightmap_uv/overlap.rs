//! A chart's triangles bucketed on a uniform grid. A triangle spanning more than
//! [`MAX_CELLS`] cells goes on a short list every query checks, so one huge floor
//! never floods the grid. Sharing an edge or a corner is not overlap.

use glam::Vec2;

use rusty_core::collections::Map;

const MAX_CELLS: i64 = 64;

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

    /// A triangle with no area is never stored.
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

    fn range(&self, tri: &[Vec2; 3]) -> Option<((i32, i32), (i32, i32))> {
        let lo = tri[0].min(tri[1]).min(tri[2]) / self.cell;
        let hi = tri[0].max(tri[1]).max(tri[2]) / self.cell;
        let (lo, hi) = (lo.floor(), hi.floor());
        let span = (hi.x - lo.x + 1.0) as i64 * (hi.y - lo.y + 1.0) as i64;
        let finite = lo.is_finite() && hi.is_finite();
        let fits = lo.abs().max_element().max(hi.abs().max_element()) < i32::MAX as f32;
        (finite && fits && span <= MAX_CELLS)
            .then_some(((lo.x as i32, lo.y as i32), (hi.x as i32, hi.y as i32)))
    }
}

fn area(tri: &[Vec2; 3]) -> f32 {
    (tri[1] - tri[0]).perp_dot(tri[2] - tri[0]).abs()
}

/// Separating-axis test; touching (within `eps`) counts as separated.
pub fn interiors_overlap(a: &[Vec2; 3], b: &[Vec2; 3], eps: f32) -> bool {
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
