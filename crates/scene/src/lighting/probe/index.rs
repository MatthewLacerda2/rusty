//! A walk out by shells of occupied cubes returns exactly what `scan` does, ties to the
//! lower index.

use glam::{IVec3, Vec3};

use super::{Probe, BLEND_PROBES};

/// The blended probes and the next, whose distance fades the farthest one out.
pub const FOUND: usize = BLEND_PROBES + 1;
use rusty_core::collections::Map;

/// Below this many a scan is as fast.
const SCAN_BELOW: usize = 32;
const PER_CELL: f32 = 2.0;
/// Cubes along an axis past which a set is too spread to grid.
const MAX_DIM: f32 = 1.0e6;

pub type Nearest = [(f32, usize); FOUND];

#[derive(Clone, Debug)]
pub struct ProbeIndex {
    origin: Vec3,
    cell: f32,
    last: IVec3,
    cells: Map<IVec3, (u32, u32)>,
    order: Vec<u32>,
    count: usize,
}

impl ProbeIndex {
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

    pub fn nearest(&self, probes: &[Probe], at: Vec3) -> Nearest {
        self.nearest_counted(probes, at).0
    }

    fn nearest_counted(&self, probes: &[Probe], at: Vec3) -> (Nearest, Option<usize>) {
        if self.count != probes.len() || !at.is_finite() {
            return (scan(probes, at), None);
        }
        let c = ((at - self.origin) / self.cell).floor();
        if c.abs().max_element() > MAX_DIM {
            return (scan(probes, at), None);
        }
        let c = c.as_ivec3();
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
            // A hair under `r` cubes, so rounding in the keys never stops the walk a shell early.
            let reach = (r as f32 - 1.0e-3).max(0.0) * self.cell;
            if near[FOUND - 1].0 < reach * reach {
                break;
            }
        }
        (near, Some(work))
    }

    fn shell(&self, c: IVec3, r: i32, mut f: impl FnMut(&[u32])) -> usize {
        let span = |a: i32, last: i32| (a - r).max(0)..=(a + r).min(last);
        let mut looked = 0;
        for z in span(c.z, self.last.z) {
            for y in span(c.y, self.last.y) {
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

const EMPTY: Nearest = [(f32::INFINITY, usize::MAX); FOUND];

pub fn scan(probes: &[Probe], at: Vec3) -> Nearest {
    let mut near = EMPTY;
    for (i, p) in probes.iter().enumerate() {
        offer(&mut near, p.position.distance_squared(at), i);
    }
    near
}

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

/// About `PER_CELL` probes a cube over the axes the set spreads along; a flat set's zero
/// cube would grid it in millimetres.
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
