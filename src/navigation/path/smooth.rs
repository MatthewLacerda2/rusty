//! String-pulling (#458): turn an A\* span path, one cell at a time, into the few
//! corners an agent actually needs to turn at.
//!
//! The rule is line-of-sight simplification: from each corner, keep extending the
//! straight run to the next span of the path while a straight walk from the corner
//! (`trace`) still crosses only linked spans and lands on that very span (so a run
//! never jumps to another floor of the same column). The last span that passed
//! becomes the next corner. Because the walk only crosses spans the bake kept, the
//! smoothed path stays inside the eroded walkable area: the agent-radius clearance
//! from walls (#277) holds without a separate margin.

use glam::Vec3;

use super::super::{NavigationGraph, SpanRef};

impl NavigationGraph {
    /// The corners of `path`, which starts at `first` and ends at `last` (points on
    /// its first and last spans). Always at least two corners for a non-empty path.
    pub(super) fn string_pull(&self, path: &[SpanRef], first: Vec3, last: Vec3) -> Vec<Vec3> {
        let Some(end) = path.len().checked_sub(1) else {
            return Vec::new();
        };
        if end == 0 {
            return vec![first, last];
        }
        let point = |i: usize| match i {
            0 => first,
            i if i == end => last,
            i => self.span_world(path[i]),
        };
        let mut corners = vec![first];
        let mut anchor = 0;
        while anchor < end {
            let mut reach = anchor + 1;
            while reach < end
                && self.sees(
                    path[anchor],
                    point(anchor),
                    path[reach + 1],
                    point(reach + 1),
                )
            {
                reach += 1;
            }
            corners.push(point(reach));
            anchor = reach;
        }
        corners
    }

    /// Whether a straight walk from `a` (standing at `pa`) reaches span `b` at `pb`.
    fn sees(&self, a: SpanRef, pa: Vec3, b: SpanRef, pb: Vec3) -> bool {
        let walk = self.trace(a, pa, pb);
        !walk.hit && walk.span == b
    }
}
