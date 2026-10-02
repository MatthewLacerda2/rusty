//! src/navigation/snap.rs — which span a world point means (#454).
//!
//! A column can hold several floors, so a point's XZ no longer names one surface.
//! The rule, used for path start and target alike, is **the nearest span below the
//! point, favouring the floor under a character's feet**: the highest span whose
//! floor is at most `max_step` above the point (a character mid-stair, or sunk a
//! little into a ramp, still resolves to the surface it stands on), else the lowest
//! span above it (a point under the whole level resolves to the bottom floor).
//! A chest-height target therefore lands on the floor under it, never the one above.
//!
//! When the point's own column holds no span (a wall, a hole, the eroded edge), the
//! search widens ring by ring up to [`SNAP_RINGS`] cells, nearest in XZ first, and
//! applies the same height rule in the cell it finds.

use glam::Vec3;

use super::{NavigationGraph, SpanRef};

/// How many rings of neighbouring cells a snap searches when the point's own column
/// holds no walkable span.
pub const SNAP_RINGS: i32 = 5;

impl NavigationGraph {
    /// The span of the point's own column the point stands on, by the height rule in
    /// the module docs, or `None` when that column holds no span.
    pub fn span_under(&self, pos: Vec3) -> Option<SpanRef> {
        let (gx, gz) = self.world_to_grid(pos);
        self.span_in_cell(gx, gz, pos.y)
    }

    /// The span a path to or from `pos` should use: [`Self::span_under`], else the
    /// nearest cell with any span within [`SNAP_RINGS`] rings.
    pub fn snap(&self, pos: Vec3) -> Option<SpanRef> {
        let (gx, gz) = self.world_to_grid(pos);
        if let Some(s) = self.span_in_cell(gx, gz, pos.y) {
            return Some(s);
        }
        for r in 1..=SNAP_RINGS {
            let mut best: Option<(i32, SpanRef)> = None;
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dz.abs() != r {
                        continue; // interior of the ring: already searched
                    }
                    let dist_sq = dx * dx + dz * dz;
                    if best.is_some_and(|(d, _)| d <= dist_sq) {
                        continue;
                    }
                    if let Some(s) = self.span_in_cell(gx + dx, gz + dz, pos.y) {
                        best = Some((dist_sq, s));
                    }
                }
            }
            if let Some((_, s)) = best {
                return Some(s);
            }
        }
        None
    }

    /// Apply the height rule within one column.
    fn span_in_cell(&self, gx: i32, gz: i32, y: f32) -> Option<SpanRef> {
        let range = self.span_range(gx, gz);
        let spans = &self.spans[range.clone()];
        // Spans are bottom-up, so the last one at or under the reach is the highest.
        let reach = y + self.max_step;
        let below = spans.iter().rposition(|s| s.y <= reach);
        let pick = below.or_else(|| (!spans.is_empty()).then_some(0))?;
        Some(SpanRef {
            gx,
            gz,
            index: (range.start + pick) as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::NavSpan;
    use super::*;

    /// One cell with floors at y = 0, 4 and 8.
    fn tower() -> NavigationGraph {
        let mut g = NavigationGraph::new(0.0, 0.0, 0.0, 0.0, 1.0);
        let floor = |y: f32, ceiling: f32| NavSpan {
            y,
            ceiling,
            area: 0,
        };
        g.spans = vec![floor(0.0, 3.8), floor(4.0, 7.8), floor(8.0, f32::INFINITY)];
        g.cell_start = vec![0, 3];
        g
    }

    fn picked(g: &NavigationGraph, y: f32) -> u32 {
        g.snap(Vec3::new(0.0, y, 0.0)).expect("a span").index
    }

    #[test]
    fn picks_the_floor_under_the_feet() {
        let g = tower();
        assert_eq!(picked(&g, 0.0), 0);
        assert_eq!(picked(&g, 1.5), 0, "chest height stays on the ground floor");
        assert_eq!(picked(&g, 4.0), 1);
        assert_eq!(picked(&g, 7.0), 1);
        assert_eq!(picked(&g, 20.0), 2);
    }

    #[test]
    fn a_point_slightly_below_a_floor_still_resolves_to_it() {
        let g = tower();
        assert_eq!(picked(&g, 3.6), 1, "within max_step under the second floor");
        assert_eq!(picked(&g, -5.0), 0, "under the level: the lowest floor");
    }

    #[test]
    fn empty_column_searches_the_nearest_ring() {
        let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        let hole = g.index(5, 5);
        g.spans.remove(hole);
        for start in &mut g.cell_start[hole + 1..] {
            *start -= 1;
        }
        assert!(g.span_under(Vec3::new(5.0, 0.0, 5.0)).is_none());
        let s = g.snap(Vec3::new(5.0, 0.0, 5.0)).expect("a neighbour");
        assert_eq!(
            (s.gx - 5).abs() + (s.gz - 5).abs(),
            1,
            "cardinal ring first"
        );
    }
}
