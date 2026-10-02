//! src/navigation/agents/changed.rs — keep a path a rebake did not touch (#456).
//!
//! An incremental rebake changes a few rectangles of cells. An agent whose
//! remaining path (from where it stands, corner to corner) crosses none of them
//! keeps its path and is restamped to the new bake generation; one that crosses
//! a changed rectangle re-plans as before. A partial path always re-plans: a
//! change anywhere may have opened the way to its target. A full bake, or a change
//! too old for the log, re-plans everyone.

use glam::{Vec2, Vec3};

use super::super::NavigationGraph;
use crate::components::NavPathStatus;
use crate::scene::NavMeshAgentComponent;

impl NavigationGraph {
    /// Restamp `agent`'s path to the current bake when no change since it was
    /// planned touches what is left of it.
    pub(super) fn keep_path_if_untouched(&self, agent: &mut NavMeshAgentComponent, at: Vec3) {
        if agent.path_generation == self.bake_generation
            || agent.cached_path.is_empty()
            || agent.path_status == NavPathStatus::Partial
        {
            return;
        }
        let Some(changed) = self.changes_since(agent.path_generation) else {
            return;
        };
        let ahead = agent.cached_path.get(agent.path_cursor..).unwrap_or(&[]);
        let points: Vec<Vec2> = std::iter::once(at)
            .chain(ahead.iter().copied())
            .map(|p| Vec2::new(p.x, p.z))
            .collect();
        let touched = changed.iter().any(|r| {
            let (lo, hi) = r.world_box(self);
            let (lo, hi) = (Vec2::new(lo.x, lo.z), Vec2::new(hi.x, hi.z));
            points
                .windows(2)
                .any(|leg| segment_hits_box(leg[0], leg[1], lo, hi))
        });
        if !touched {
            agent.path_generation = self.bake_generation;
        }
    }
}

/// Whether the segment `a..b` meets the box `lo..hi` (slab test, edges included).
fn segment_hits_box(a: Vec2, b: Vec2, lo: Vec2, hi: Vec2) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for axis in 0..2 {
        if d[axis] == 0.0 {
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return false;
            }
            continue;
        }
        let ta = (lo[axis] - a[axis]) / d[axis];
        let tb = (hi[axis] - a[axis]) / d[axis];
        t0 = t0.max(ta.min(tb));
        t1 = t1.min(ta.max(tb));
        if t0 > t1 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_box_hits_crossings_and_misses_bystanders() {
        let (lo, hi) = (Vec2::new(2.0, 2.0), Vec2::new(4.0, 4.0));
        let hit = |a: (f32, f32), b: (f32, f32)| {
            segment_hits_box(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1), lo, hi)
        };
        assert!(hit((0.0, 3.0), (6.0, 3.0)), "straight through");
        assert!(hit((3.0, 3.0), (3.0, 3.0)), "a point inside");
        assert!(hit((0.0, 0.0), (5.0, 5.0)), "diagonally through");
        assert!(!hit((0.0, 5.0), (6.0, 5.0)), "passes above");
        assert!(!hit((0.0, 0.0), (1.9, 1.9)), "stops short");
        assert!(!hit((0.0, 3.0), (1.0, 6.0)), "slants past");
    }
}
