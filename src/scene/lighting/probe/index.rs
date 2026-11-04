//! A spatial index over a probe set's positions (#1737), so sampling a set with no
//! grid (the automatic placement's) reads the few probes near a point instead of
//! scanning all of them.
//!
//! Probes are bucketed in a uniform grid of cubes, about two to a cube, held in a
//! hash of the occupied cubes only (`core::collections`, no seed). A query walks
//! shells of cubes outwards from the point's own and stops once no unvisited cube
//! can hold a nearer probe, so it returns exactly what the full scan returns: the
//! [`FOUND`] nearest by distance, ties to the lower index. When the walk
//! would visit more cubes than the set has probes (a point far outside a sparse
//! set), it scans instead. Built when a set is first sampled after it changed,
//! never saved.

use glam::{IVec3, Vec3};

use super::{Probe, BLEND_PROBES};

/// How many nearest probes a query finds: the [`BLEND_PROBES`] blended and the one
/// past them, whose distance fades the farthest blended one out (`ProbeVolume::blend`).
pub const FOUND: usize = BLEND_PROBES + 1;
use crate::core::collections::Map;

/// Below this many probes a scan is as fast as the index.
const SCAN_BELOW: usize = 32;
/// Probes a cube holds on average.
const PER_CELL: f32 = 2.0;
/// The most cubes along an axis: past it the set is too spread to grid.
const MAX_DIM: f32 = 1.0e6;

/// The nearest probes found so far: `(squared distance, index)`, sorted by both,
/// unfilled slots `(INFINITY, usize::MAX)`.
pub type Nearest = [(f32, usize); FOUND];

/// The bucketed probe set: each occupied cube's run in `order`.
#[derive(Clone, Debug)]
pub struct ProbeIndex {
    origin: Vec3,
    cell: f32,
    /// The last cube along each axis (cubes run `0..=last`).
    last: IVec3,
    cells: Map<IVec3, (u32, u32)>,
    order: Vec<u32>,
    /// How many probes the set had when built: another count means a stale index.
    count: usize,
}

impl ProbeIndex {
    /// The index of `probes`, `None` when a scan does as well (a small set, or one
    /// too spread to grid). Probes at a non-finite position are left out, as the
    /// scan never picks them.
    pub fn build(probes: &[Probe]) -> Option<Self> {
        let finite = || (probes.iter().enumerate()).filter(|(_, p)| p.position.is_finite());
        if finite().count() < SCAN_BELOW {
            return None;
        }
        let (lo, hi) = finite().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), (_, p)| {
            (lo.min(p.position), hi.max(p.position))
        });
        let cell = cell_size(hi - lo, finite().count());
        let span = (hi - lo) / cell;
        if !cell.is_finite() || span.max_element() > MAX_DIM {
            return None;
        }
        let key = |p: Vec3| ((p - lo) / cell).floor().as_ivec3();
        let mut keyed: Vec<(IVec3, u32)> =
            finite().map(|(i, p)| (key(p.position), i as u32)).collect();
        keyed.sort_by_key(|&(k, i)| (k.z, k.y, k.x, i));
        let mut cells = Map::default();
        for (n, &(k, _)) in keyed.iter().enumerate() {
            cells.entry(k).or_insert((n as u32, 0)).1 += 1;
        }
        Some(Self {
            origin: lo,
            cell,
            last: key(hi),
            cells,
            order: keyed.into_iter().map(|(_, i)| i).collect(),
            count: probes.len(),
        })
    }

    /// The [`FOUND`] nearest of `probes` to `at`, as [`scan`] finds them.
    pub fn nearest(&self, probes: &[Probe], at: Vec3) -> Nearest {
        self.nearest_counted(probes, at).0
    }

    /// [`nearest`](Self::nearest) and its cost: the cubes looked at, or `None` when
    /// it scanned instead.
    fn nearest_counted(&self, probes: &[Probe], at: Vec3) -> (Nearest, Option<usize>) {
        if self.count != probes.len() || !at.is_finite() {
            return (scan(probes, at), None);
        }
        let c = ((at - self.origin) / self.cell).floor();
        if c.abs().max_element() > MAX_DIM {
            return (scan(probes, at), None);
        }
        let c = c.as_ivec3();
        // The first shell that can hold a cube; the last that does.
        let outside = (IVec3::ZERO - c).max(c - self.last).max(IVec3::ZERO);
        let farthest = c.abs().max((c - self.last).abs());
        let (first, last) = (outside.max_element(), farthest.max_element());
        let mut near = EMPTY;
        let mut work = 0;
        for r in first..=last {
            work += self.shell(c, r, |run| {
                for &i in run {
                    offer(
                        &mut near,
                        probes[i as usize].position.distance_squared(at),
                        i as usize,
                    );
                }
            });
            if work > self.order.len() {
                return (scan(probes, at), None);
            }
            // Every cube not yet looked at is at least `r` cubes away; a hair less,
            // so rounding in the keys never stops the walk a shell early.
            let reach = (r as f32 - 1.0e-3).max(0.0) * self.cell;
            if near[FOUND - 1].0 < reach * reach {
                break;
            }
        }
        (near, Some(work))
    }

    /// Call `f` with the probe run of every occupied cube in the shell `r` cubes out
    /// from `c` (Chebyshev distance), within the grid; returns the cubes looked at.
    fn shell(&self, c: IVec3, r: i32, mut f: impl FnMut(&[u32])) -> usize {
        let span = |a: i32, last: i32| (a - r).max(0)..=(a + r).min(last);
        let mut looked = 0;
        for z in span(c.z, self.last.z) {
            for y in span(c.y, self.last.y) {
                // Inside the shell's faces only its two x ends are on it.
                let inner = (z - c.z).abs() < r && (y - c.y).abs() < r;
                let (mut x, end) = span(c.x, self.last.x).into_inner();
                while x <= end {
                    if inner && (x - c.x).abs() < r {
                        x = c.x + r;
                        continue;
                    }
                    looked += 1;
                    if let Some(&(start, len)) = self.cells.get(&IVec3::new(x, y, z)) {
                        f(&self.order[start as usize..(start + len) as usize]);
                    }
                    x += 1;
                }
            }
        }
        looked
    }
}

/// No probe found yet.
const EMPTY: Nearest = [(f32::INFINITY, usize::MAX); FOUND];

/// The [`FOUND`] nearest of `probes` to `at` by a full scan: the answer the
/// index must give.
pub fn scan(probes: &[Probe], at: Vec3) -> Nearest {
    let mut near = EMPTY;
    for (i, p) in probes.iter().enumerate() {
        offer(&mut near, p.position.distance_squared(at), i);
    }
    near
}

/// Keep probe `i` at squared distance `d` if it is among the nearest: by distance,
/// then by index. A non-finite distance never is.
fn offer(near: &mut Nearest, d: f32, i: usize) {
    let worst = near[FOUND - 1];
    if !d.is_finite() || !(d < worst.0 || (d == worst.0 && i < worst.1)) {
        return;
    }
    let mut at = FOUND - 1;
    while at > 0 && (d < near[at - 1].0 || (d == near[at - 1].0 && i < near[at - 1].1)) {
        near[at] = near[at - 1];
        at -= 1;
    }
    near[at] = (d, i);
}

/// A cube's edge for `count` probes across `extent`: about [`PER_CELL`] probes a
/// cube, over the axes the set spreads along (a flat layout grids in 2D, a line
/// in 1D). An arm whose size is zero is not taken: a flat set's cube is 0 m, and
/// `0 ≥ 0` would grid it in millimetres.
fn cell_size(extent: Vec3, count: usize) -> f32 {
    let mut e = extent.to_array();
    e.sort_by(|a, b| b.total_cmp(a));
    let per = PER_CELL / count as f32;
    let cube = (e[0] * e[1] * e[2] * per).cbrt();
    let square = (e[0] * e[1] * per).sqrt();
    let cell = match () {
        _ if cube > 0.0 && e[2] >= cube => cube,
        _ if square > 0.0 && e[1] >= square => square,
        _ => e[0] * per,
    };
    cell.max(1.0e-3)
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
