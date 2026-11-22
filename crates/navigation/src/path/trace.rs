//! The segment is stepped cell by cell in XZ (a grid DDA over the cell squares, whose edges sit
//! half a cell from each centre). Where it passes exactly through a cell corner it is a diagonal
//! move, and both cells beside the corner must be reachable too (no corner-cutting). A span failing
//! the area filter stops the walk like a wall; the start span is exempt.

use glam::Vec3;

use super::super::{in_mask, NavigationGraph, SpanRef};

/// In segment parameter `t`: two crossings this close are one corner.
const CORNER_EPSILON: f32 = 1e-5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavRaycastHit {
    pub hit: bool,
    pub position: Vec3,
    pub span: SpanRef,
}

impl NavigationGraph {
    /// Off the navmesh the walk is blocked at once, at `from` (Unity reports a hit for a source off
    /// the mesh).
    pub fn raycast_masked(&self, from: Vec3, to: Vec3, mask: u32) -> Option<NavRaycastHit> {
        match self.span_under(from) {
            Some(start) => Some(self.trace(start, from, to, |a| in_mask(mask, a))),
            None => self.snap(from).map(|span| NavRaycastHit {
                hit: true,
                position: from,
                span,
            }),
        }
    }

    pub(super) fn trace(
        &self,
        start: SpanRef,
        from: Vec3,
        to: Vec3,
        allowed: impl Fn(u8) -> bool,
    ) -> NavRaycastHit {
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
            let next = self.neighbour(on, dx, dz);
            let next = next.filter(|n| self.walk_allows(on, *n, (dx, dz), &allowed));
            let Some(next) = next else {
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

    fn walk_allows(
        &self,
        on: SpanRef,
        next: SpanRef,
        (dx, dz): (i32, i32),
        allowed: &impl Fn(u8) -> bool,
    ) -> bool {
        let passes =
            |s: Option<SpanRef>| s.is_some_and(|s| allowed(self.spans[s.index as usize].area));
        passes(Some(next))
            && (dx == 0
                || dz == 0
                || passes(self.link_to(on, on.gx + dx, on.gz))
                    && passes(self.link_to(on, on.gx, on.gz + dz)))
    }

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
