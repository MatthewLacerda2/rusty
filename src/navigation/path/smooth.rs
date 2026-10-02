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
//!
//! Areas (#460): the walked stretches are cut again wherever the area cost changes
//! along the path, at the last span before the change, and each run is pulled with
//! a walk that may cross only spans the agent's mask allows and that cost no more
//! than the run's own. A straight run therefore never cuts across a region the
//! agent may not enter, nor across a costlier area A\* routed around; on a path of
//! one cost this changes nothing.

use glam::Vec3;

use super::super::{in_mask, NavigationGraph, OffMeshLinkData, SpanRef};

/// A route's corners, and the off-mesh legs among them (see `NavPath::links`).
type Route = (Vec<Vec3>, Vec<(usize, OffMeshLinkData)>);

impl NavigationGraph {
    /// The corners of a span path that may cross off-mesh links (#462). The walked
    /// stretches between links are string-pulled on their own, so a straight run
    /// never cuts across a link; each link is one leg from its start to its end.
    pub(super) fn pull_route(&self, path: &[SpanRef], ends: (Vec3, Vec3), mask: u32) -> Route {
        let (first, last) = ends;
        let (mut corners, mut links) = (Vec::new(), Vec::new());
        let (mut from, mut at) = (0, first);
        for i in 1..path.len() {
            if self.is_walk(path[i - 1], path[i]) {
                continue;
            }
            let Some(link) = self.link_between(path[i - 1], path[i]) else {
                continue;
            };
            corners.extend(self.pull_areas(&path[from..i], (at, link.start), mask));
            links.push((corners.len(), link));
            (from, at) = (i, link.end);
        }
        corners.extend(self.pull_areas(&path[from..], (at, last), mask));
        (corners, links)
    }

    /// The corners of a walked stretch between `ends`, string-pulled one run of equal
    /// area cost at a time. A run ends on the last span before the cost changes; the
    /// next run starts from that span, so the two share that corner.
    fn pull_areas(&self, path: &[SpanRef], ends: (Vec3, Vec3), mask: u32) -> Vec<Vec3> {
        let cost = |i: usize| self.area_cost(self.spans[path[i].index as usize].area);
        let mut corners: Vec<Vec3> = Vec::new();
        let (mut from, mut at) = (0, ends.0);
        let mut pull = |from: usize, run: (Vec3, Vec3), to: usize, max_cost: f32| {
            let run = self.string_pull(&path[from..to], run, (mask, max_cost));
            corners.extend(run.into_iter().skip(usize::from(from > 0)));
        };
        for k in 1..path.len() {
            if cost(k) != cost(k - 1) {
                let anchor = self.span_world(path[k - 1]);
                pull(from, (at, anchor), k, cost(k - 1));
                (from, at) = (k - 1, anchor);
            }
        }
        if let Some(end) = path.len().checked_sub(1) {
            pull(from, (at, ends.1), path.len(), cost(end));
        }
        corners
    }

    /// Whether `b` is one walking step from `a` (`neighbour`'s rule).
    fn is_walk(&self, a: SpanRef, b: SpanRef) -> bool {
        let (dx, dz) = (b.gx - a.gx, b.gz - a.gz);
        dx.abs() <= 1 && dz.abs() <= 1 && self.neighbour(a, dx, dz) == Some(b)
    }

    /// The corners of `path`, which starts at `first` and ends at `last` (points on
    /// its first and last spans), pulled across spans the `(mask, max cost)` filter
    /// allows. Always at least two corners for a non-empty path.
    fn string_pull(
        &self,
        path: &[SpanRef],
        (first, last): (Vec3, Vec3),
        filter: (u32, f32),
    ) -> Vec<Vec3> {
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
                    (path[anchor], point(anchor)),
                    (path[reach + 1], point(reach + 1)),
                    filter,
                )
            {
                reach += 1;
            }
            corners.push(point(reach));
            anchor = reach;
        }
        corners
    }

    /// Whether a straight walk from span `a` (standing at `pa`), entering only areas in
    /// `mask` costing at most `max_cost`, reaches span `b` at `pb`.
    fn sees(&self, (a, pa): (SpanRef, Vec3), (b, pb): (SpanRef, Vec3), filter: (u32, f32)) -> bool {
        let (mask, max_cost) = filter;
        let walk = self.trace(a, pa, pb, |s| {
            in_mask(mask, s) && self.area_cost(s) <= max_cost
        });
        !walk.hit && walk.span == b
    }
}
