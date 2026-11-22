//! Unity's NavMesh query set, answered on the navmesh rather than physics: "walkable", not "solid".
//! Every query is synchronous and a pure function of the baked graph, so it stays deterministic.

#[cfg(test)]
mod area_tests;
mod sample;
mod smooth;
#[cfg(test)]
mod tests;
mod trace;

use glam::Vec3;

pub use rusty_components::NavPathStatus;
pub use trace::NavRaycastHit;

use super::{NavigationGraph, OffMeshLinkData, SpanRef, ALL_AREAS};

/// Unity's `NavMeshPath`; `corners` runs from the start to the end, both included.
#[derive(Clone, Debug, PartialEq)]
pub struct NavPath {
    pub status: NavPathStatus,
    pub corners: Vec<Vec3>,
    /// `(i, link)`: the leg from corner `i - 1` to corner `i` is `link`.
    pub links: Vec<(usize, OffMeshLinkData)>,
}

impl NavPath {
    pub fn invalid() -> Self {
        Self {
            status: NavPathStatus::Invalid,
            corners: Vec::new(),
            links: Vec::new(),
        }
    }
}

pub fn path_length(corners: &[Vec3]) -> f32 {
    corners.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// A fraction of the cell size, so a clamped point maps back to that cell, not its neighbour.
const CELL_INSET: f32 = 1e-3;

impl NavigationGraph {
    pub fn calculate_path(&self, from: Vec3, to: Vec3) -> NavPath {
        self.calculate_path_masked(from, to, ALL_AREAS)
    }

    pub fn calculate_path_masked(&self, from: Vec3, to: Vec3, mask: u32) -> NavPath {
        let (Some(start), Some(goal)) = (self.snap(from), self.snap(to)) else {
            return NavPath::invalid();
        };
        let (spans, complete) = self.find_path_masked(start, goal, mask);
        self.route_path((from, to), &spans, complete, mask)
    }

    pub(crate) fn route_path(
        &self,
        (from, to): (Vec3, Vec3),
        spans: &[SpanRef],
        complete: bool,
        mask: u32,
    ) -> NavPath {
        let (Some(&start), Some(&end)) = (spans.first(), spans.last()) else {
            return NavPath::invalid();
        };
        let first = self.point_on_span(from, start);
        let last = match complete {
            true => self.point_on_span(to, end),
            false => self.span_world(end),
        };
        let (corners, links) = self.pull_route(spans, (first, last), mask);
        NavPath {
            status: if complete {
                NavPathStatus::Complete
            } else {
                NavPathStatus::Partial
            },
            corners,
            links,
        }
    }

    fn point_on_span(&self, p: Vec3, s: SpanRef) -> Vec3 {
        if self.span_under(p) == Some(s) {
            Vec3::new(p.x, self.spans[s.index as usize].y, p.z)
        } else {
            self.span_world(s)
        }
    }

    fn clamp_to_cell(&self, p: Vec3, gx: i32, gz: i32) -> Vec3 {
        let c = self.cell_center(gx, gz);
        let h = self.grid_spacing * (0.5 - CELL_INSET);
        Vec3::new(
            p.x.clamp(c.x - h, c.x + h),
            p.y,
            p.z.clamp(c.z - h, c.z + h),
        )
    }
}
