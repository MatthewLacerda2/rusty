//! Line-of-sight string-pulling: from each corner, extend the run while a straight walk (`trace`)
//! still crosses only linked spans and lands on that very span, so a run never jumps to another
//! floor of the same column. Because the walk only crosses spans the bake kept, the agent-radius
//! clearance from walls holds without a separate margin. Runs are cut where the area cost changes,
//! so a straight run never cuts across a region the agent may not enter or a costlier area A\*
//! routed around.

use glam::Vec3;

use super::super::{in_mask, NavigationGraph, OffMeshLinkData, SpanRef};

type Route = (Vec<Vec3>, Vec<(usize, OffMeshLinkData)>);

impl NavigationGraph {
    /// Stretches between links are pulled on their own.
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

    fn is_walk(&self, a: SpanRef, b: SpanRef) -> bool {
        let (dx, dz) = (b.gx - a.gx, b.gz - a.gz);
        dx.abs() <= 1 && dz.abs() <= 1 && self.neighbour(a, dx, dz) == Some(b)
    }

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

    fn sees(&self, (a, pa): (SpanRef, Vec3), (b, pb): (SpanRef, Vec3), filter: (u32, f32)) -> bool {
        let (mask, max_cost) = filter;
        let walk = self.trace(a, pa, pb, |s| {
            in_mask(mask, s) && self.area_cost(s) <= max_cost
        });
        !walk.hit && walk.span == b
    }
}
