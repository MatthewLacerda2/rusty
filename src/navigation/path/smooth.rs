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
//! Areas (#460): the walked stretches are cut again wherever the path changes area,
//! at the last span before the change, and each run is pulled with a walk that may
//! cross only spans of the run's own area. A straight run therefore never cuts
//! across a region the agent may not enter, nor across a costlier area A\* routed
//! around; on a single-area path this changes nothing.

use glam::Vec3;

use super::super::{NavigationGraph, OffMeshLinkData, SpanRef};

/// A route's corners, and the off-mesh legs among them (see `NavPath::links`).
type Route = (Vec<Vec3>, Vec<(usize, OffMeshLinkData)>);

impl NavigationGraph {
    /// The corners of a span path that may cross off-mesh links (#462). The walked
    /// stretches between links are string-pulled on their own, so a straight run
    /// never cuts across a link; each link is one leg from its start to its end.
    pub(super) fn pull_route(&self, path: &[SpanRef], first: Vec3, last: Vec3) -> Route {
        let (mut corners, mut links) = (Vec::new(), Vec::new());
        let (mut from, mut at) = (0, first);
        for i in 1..path.len() {
            if self.is_walk(path[i - 1], path[i]) {
                continue;
            }
            let Some(link) = self.link_between(path[i - 1], path[i]) else {
                continue;
            };
            corners.extend(self.pull_areas(&path[from..i], at, link.start));
            links.push((corners.len(), link));
            (from, at) = (i, link.end);
        }
        corners.extend(self.pull_areas(&path[from..], at, last));
        (corners, links)
    }

    /// The corners of a walked stretch from `first` to `last`, string-pulled one area
    /// run at a time. A run ends on the last span before the area changes; the next
    /// run starts from that span, so the two share that corner.
    fn pull_areas(&self, path: &[SpanRef], first: Vec3, last: Vec3) -> Vec<Vec3> {
        let area = |i: usize| self.spans[path[i].index as usize].area;
        let mut corners: Vec<Vec3> = Vec::new();
        let (mut from, mut at) = (0, first);
        for k in 1..path.len() {
            if area(k) != area(k - 1) {
                let anchor = self.span_world(path[k - 1]);
                let run = self.string_pull(&path[from..k], at, anchor, area(k - 1));
                corners.extend(run.into_iter().skip(usize::from(from > 0)));
                (from, at) = (k - 1, anchor);
            }
        }
        let area = path.last().map_or(0, |s| self.spans[s.index as usize].area);
        let run = self.string_pull(&path[from..], at, last, area);
        corners.extend(run.into_iter().skip(usize::from(from > 0)));
        corners
    }

    /// Whether `b` is one walking step from `a` (`neighbour`'s rule).
    fn is_walk(&self, a: SpanRef, b: SpanRef) -> bool {
        let (dx, dz) = (b.gx - a.gx, b.gz - a.gz);
        dx.abs() <= 1 && dz.abs() <= 1 && self.neighbour(a, dx, dz) == Some(b)
    }

    /// The corners of `path`, which starts at `first` and ends at `last` (points on
    /// its first and last spans), pulled across spans of `area` only. Always at least
    /// two corners for a non-empty path.
    fn string_pull(&self, path: &[SpanRef], first: Vec3, last: Vec3, area: u8) -> Vec<Vec3> {
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
                    area,
                )
            {
                reach += 1;
            }
            corners.push(point(reach));
            anchor = reach;
        }
        corners
    }

    /// Whether a straight walk from span `a` (standing at `pa`) across spans of `area`
    /// reaches span `b` at `pb`.
    fn sees(&self, (a, pa): (SpanRef, Vec3), (b, pb): (SpanRef, Vec3), area: u8) -> bool {
        let walk = self.trace(a, pa, pb, |s| s == area);
        !walk.hit && walk.span == b
    }
}
