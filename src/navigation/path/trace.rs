//! A straight walk across the span grid (#458): Unity's `NavMesh.Raycast`, and the
//! visibility test the smoothing uses.
//!
//! The segment is stepped cell by cell in XZ (a grid DDA over the cell squares, whose
//! edges sit half a cell from each centre). Every cell crossed must link to the span
//! the walk is on (`links.rs`: step, slope and headroom), so the walk follows a ramp
//! or a stair onto the next floor and stops at a wall, a ledge or an eroded edge.
//! Where the segment passes exactly through a cell corner it is a diagonal move, and
//! both cells beside the corner must be reachable too (no corner-cutting).

use glam::Vec3;

use super::super::{NavigationGraph, SpanRef};

/// Two crossings closer than this (in segment parameter `t`) count as one corner.
const CORNER_EPSILON: f32 = 1e-5;

/// The result of a navmesh raycast (Unity's `NavMeshHit` for `NavMesh.Raycast`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavRaycastHit {
    /// Whether the walk was stopped before reaching the end point.
    pub hit: bool,
    /// Where the walk stopped (the end point when clear), on the floor it reached.
    pub position: Vec3,
    /// The span the walk stopped on.
    pub span: SpanRef,
}

impl NavigationGraph {
    /// Walk the navmesh straight from `from` toward `to`. `from` must stand on the
    /// navmesh (`span_under`); off it the walk is blocked at once, at `from` (Unity
    /// reports a hit for a source off the mesh). `None` when the graph is empty there.
    pub fn raycast(&self, from: Vec3, to: Vec3) -> Option<NavRaycastHit> {
        match self.span_under(from) {
            Some(start) => Some(self.trace(start, from, to)),
            None => self.snap(from).map(|span| NavRaycastHit {
                hit: true,
                position: from,
                span,
            }),
        }
    }

    /// Walk from span `start`, standing at `from`, straight toward `to`.
    pub(super) fn trace(&self, start: SpanRef, from: Vec3, to: Vec3) -> NavRaycastHit {
        let s = self.grid_spacing;
        let (fu, fv) = ((from.x - self.min_x) / s, (from.z - self.min_z) / s);
        let (du, dv) = ((to.x - from.x) / s, (to.z - from.z) / s);
        let (step_x, mut next_x, delta_x) = axis(start.gx, fu, du);
        let (step_z, mut next_z, delta_z) = axis(start.gz, fv, dv);
        let mut on = start;
        loop {
            let t = next_x.min(next_z);
            if t > 1.0 {
                return self.stop(on, from.lerp(to, 1.0), false);
            }
            let corner = (next_x - next_z).abs() <= CORNER_EPSILON;
            let (dx, dz) = if corner {
                (step_x, step_z)
            } else if next_x < next_z {
                (step_x, 0)
            } else {
                (0, step_z)
            };
            let Some(next) = self.neighbour(on, dx, dz) else {
                return self.stop(on, from.lerp(to, t), true);
            };
            on = next;
            if dx != 0 {
                next_x += delta_x;
            }
            if dz != 0 {
                next_z += delta_z;
            }
        }
    }

    /// The walk's result on span `on`: `p` kept inside that span's cell, on its floor.
    fn stop(&self, on: SpanRef, p: Vec3, hit: bool) -> NavRaycastHit {
        let mut position = if hit {
            self.clamp_to_cell(p, on.gx, on.gz)
        } else {
            p
        };
        position.y = self.spans[on.index as usize].y;
        NavRaycastHit {
            hit,
            position,
            span: on,
        }
    }
}

/// One axis of the DDA: the cell step direction, the segment parameter of the first
/// cell edge crossed, and the parameter between successive edges. `cell` is the
/// start cell, `from` the start in cell units, `d` the segment's extent in cells.
fn axis(cell: i32, from: f32, d: f32) -> (i32, f32, f32) {
    if d == 0.0 {
        return (0, f32::INFINITY, f32::INFINITY);
    }
    let (step, edge) = if d > 0.0 {
        (1, cell as f32 + 0.5)
    } else {
        (-1, cell as f32 - 0.5)
    };
    (step, ((edge - from) / d).max(0.0), (1.0 / d).abs())
}
