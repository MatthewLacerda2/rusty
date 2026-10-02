//! src/navigation/bake/heightfield.rs — solid spans → the compact walkable spans (#454).
//!
//! Recast's heightfield and compact-heightfield steps, with its low-height filter:
//!
//! 1. **Merge.** Each column's solids are sorted bottom-up and merged where they
//!    overlap or touch, so a crate resting on a floor is one solid. The merged top
//!    keeps the walkable flag of whichever solid reaches it; solids whose tops lie
//!    within `max_step` of each other OR their flags (Recast's flag-merge threshold,
//!    so a low unwalkable lip doesn't spoil the floor it sits in).
//! 2. **Open space.** A walkable top becomes a span whose ceiling is the bottom of
//!    the next solid up (`∞` with none).
//! 3. **Headroom (#278).** A span with less than `agent_height` of open space is
//!    dropped: nobody crawls under a low overhang.
//!
//! Sorting uses a total order on `(cell, min, max, walkable)`, so the result is a
//! pure function of the geometry, whatever order the colliders were visited in.

use super::super::{NavSpan, NavigationGraph};
use super::raster::Solid;

/// Solids closer than this vertically are touching, and merge.
const TOUCH_EPS: f32 = 1e-3;

impl NavigationGraph {
    /// Replace the graph's spans with the walkable open space above `solids`.
    pub(super) fn build_spans(&mut self, mut solids: Vec<Solid>) {
        solids.sort_by(|a, b| {
            a.cell
                .cmp(&b.cell)
                .then(a.min.total_cmp(&b.min))
                .then(a.max.total_cmp(&b.max))
                .then(a.walkable.cmp(&b.walkable))
        });
        let cells = (self.width * self.height) as usize;
        self.spans.clear();
        self.cell_start.clear();
        self.cell_start.reserve(cells + 1);
        let mut next = 0;
        let mut column = Vec::new();
        for cell in 0..cells as u32 {
            self.cell_start.push(self.spans.len() as u32);
            let end = next + solids[next..].partition_point(|s| s.cell == cell);
            merge_column(&solids[next..end], self.max_step, &mut column);
            self.push_open_spans(&column);
            next = end;
        }
        self.cell_start.push(self.spans.len() as u32);
    }

    /// Append the walkable spans above a merged column that clear `agent_height`.
    fn push_open_spans(&mut self, column: &[Solid]) {
        for (i, s) in column.iter().enumerate() {
            if !s.walkable {
                continue;
            }
            let ceiling = column.get(i + 1).map_or(f32::INFINITY, |above| above.min);
            if ceiling - s.max >= self.agent_height {
                self.spans.push(NavSpan { y: s.max, ceiling });
            }
        }
    }
}

/// Merge one column's sorted solids into `out` (cleared first).
fn merge_column(solids: &[Solid], flag_merge: f32, out: &mut Vec<Solid>) {
    out.clear();
    for &s in solids {
        match out.last_mut() {
            Some(last) if s.min <= last.max + TOUCH_EPS => {
                let top = last.max.max(s.max);
                last.walkable = (last.walkable && last.max >= top - flag_merge)
                    || (s.walkable && s.max >= top - flag_merge);
                last.max = top;
            }
            _ => out.push(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(min: f32, max: f32, walkable: bool) -> Solid {
        Solid {
            cell: 0,
            min,
            max,
            walkable,
        }
    }

    #[test]
    fn touching_solids_merge_and_the_higher_top_decides() {
        let mut out = Vec::new();
        let floor = solid(-0.1, 0.0, true);
        let crate_ = solid(0.0, 1.0, false);
        merge_column(&[floor, crate_], 0.5, &mut out);
        assert_eq!(out, vec![solid(-0.1, 1.0, false)]);
    }

    #[test]
    fn a_low_unwalkable_lip_keeps_the_floor_walkable() {
        let mut out = Vec::new();
        merge_column(
            &[solid(-0.1, 0.0, true), solid(-0.1, 0.2, false)],
            0.5,
            &mut out,
        );
        assert!(
            out[0].walkable,
            "a 0.2 lip is within the flag-merge threshold"
        );
        assert_eq!(out[0].max, 0.2);
    }

    #[test]
    fn a_gap_keeps_two_solids() {
        let mut out = Vec::new();
        merge_column(
            &[solid(-0.1, 0.0, true), solid(3.0, 3.2, true)],
            0.5,
            &mut out,
        );
        assert_eq!(out.len(), 2);
    }
}
