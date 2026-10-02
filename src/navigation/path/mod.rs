//! src/navigation/path/ — path queries over the span grid (#458): the smoothed
//! corner path (`CalculatePath`), the nearest walkable point (`SamplePosition`) and
//! the walkable straight line (`Raycast`). Unity's NavMesh query set, answered on the
//! navmesh rather than physics: "walkable", not "solid". Every query is synchronous
//! and a pure function of the baked graph, so it stays deterministic.
//!
//! Split by responsibility: `trace` walks a straight line across linked spans (the
//! raycast and the smoothing's visibility test), `smooth` string-pulls an A\* span
//! path into corners (cut at every off-mesh link, #462, which it never pulls a
//! string across), `sample` finds the nearest walkable point.

mod sample;
mod smooth;
#[cfg(test)]
mod tests;
mod trace;

use glam::Vec3;

pub use crate::components::NavPathStatus;
pub use trace::NavRaycastHit;

use super::{NavigationGraph, OffMeshLinkData, SpanRef, ALL_AREAS};

/// A computed path: Unity's `NavMeshPath`. `corners` runs from the start to the end,
/// both included, and only turns where something is in the way.
#[derive(Clone, Debug, PartialEq)]
pub struct NavPath {
    pub status: NavPathStatus,
    pub corners: Vec<Vec3>,
    /// The off-mesh links the path crosses (#462): `(i, link)` means the leg from
    /// corner `i - 1` to corner `i` is `link`, from its start to its end.
    pub links: Vec<(usize, OffMeshLinkData)>,
}

impl NavPath {
    /// No path: an end has no navmesh nearby.
    pub fn invalid() -> Self {
        Self {
            status: NavPathStatus::Invalid,
            corners: Vec::new(),
            links: Vec::new(),
        }
    }

    /// The path's length along its corners, in world units (0 for no path).
    pub fn length(&self) -> f32 {
        path_length(&self.corners)
    }
}

/// The length of a polyline through `corners`, in world units.
pub fn path_length(corners: &[Vec3]) -> f32 {
    corners.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// How far inside a cell's square a clamped point stays, as a fraction of the cell
/// size, so the point maps back to that cell rather than its neighbour.
const CELL_INSET: f32 = 1e-3;

impl NavigationGraph {
    /// The path from `from` to `to`, smoothed into corners. Both ends snap to spans
    /// by the `snap.rs` rule; when the target's span is unreachable the path ends on
    /// the nearest reachable span instead (`Partial`), and when either end has no
    /// navmesh nearby there is no path (`Invalid`, no corners).
    pub fn calculate_path(&self, from: Vec3, to: Vec3) -> NavPath {
        self.calculate_path_masked(from, to, ALL_AREAS)
    }

    /// [`Self::calculate_path`] entering only the areas in `mask` (#460). The ends
    /// snap as usual; a target in an excluded area gives a `Partial` path to the
    /// nearest point the agent may reach.
    pub fn calculate_path_masked(&self, from: Vec3, to: Vec3, mask: u32) -> NavPath {
        let (Some(start), Some(goal)) = (self.snap(from), self.snap(to)) else {
            return NavPath::invalid();
        };
        let (spans, complete) = self.find_path_masked(start, goal, mask);
        let first = self.point_on_span(from, start);
        let last = match spans.last() {
            Some(&end) if complete => self.point_on_span(to, end),
            Some(&end) => self.span_world(end),
            None => first,
        };
        let (corners, links) = self.pull_route(&spans, first, last);
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

    /// `p` on the floor of span `s` when `p` lies over that span's own cell, else the
    /// span's cell centre: the end of a path is the query point itself where it can be.
    fn point_on_span(&self, p: Vec3, s: SpanRef) -> Vec3 {
        if self.span_under(p) == Some(s) {
            Vec3::new(p.x, self.spans[s.index as usize].y, p.z)
        } else {
            self.span_world(s)
        }
    }

    /// `p`'s XZ clamped into cell `(gx, gz)`'s square (inset by [`CELL_INSET`]), so
    /// `world_to_grid` maps the result back to that cell.
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
