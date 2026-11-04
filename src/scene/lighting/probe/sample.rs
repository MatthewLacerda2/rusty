//! src/scene/lighting/probe/sample.rs — reading the probe field at a point:
//! trilinear over the grid, else the [`BLEND_PROBES`] nearest probes blended.

use glam::Vec3;

use super::index::{self, ProbeIndex};
use super::{ProbeGrid, ProbeVolume, BLEND_PROBES, ON_PROBE};
use crate::scene::lighting::sh::Sh9;

impl ProbeVolume {
    /// Sample the interpolated SH irradiance at a world position. Trilinear over the
    /// grid when one is present and the point falls inside it; otherwise (free probes,
    /// or a point outside the grid) the [`BLEND_PROBES`] nearest blended. `None` when
    /// there are no probes.
    pub fn sample(&self, position: Vec3) -> Option<Sh9> {
        if self.probes.is_empty() {
            return None;
        }
        if let Some(grid) = &self.grid {
            if let Some(sh) = self.sample_trilinear(grid, position) {
                return Some(sh);
            }
        }
        Some(self.sample_nearest(position))
    }

    /// Trilinear interpolation within the grid; `None` if the point lies outside the
    /// lattice bounds (the caller then falls back to nearest).
    fn sample_trilinear(&self, grid: &ProbeGrid, position: Vec3) -> Option<Sh9> {
        let local = (position - grid.origin) / grid.spacing;
        let max = Vec3::new(
            (grid.counts[0].saturating_sub(1)) as f32,
            (grid.counts[1].saturating_sub(1)) as f32,
            (grid.counts[2].saturating_sub(1)) as f32,
        );
        if local.cmplt(Vec3::ZERO).any() || local.cmpgt(max).any() {
            return None;
        }
        let base = local.floor();
        let frac = local - base;
        let (i0, j0, k0) = (base.x as u32, base.y as u32, base.z as u32);
        let i1 = (i0 + 1).min(grid.counts[0].saturating_sub(1));
        let j1 = (j0 + 1).min(grid.counts[1].saturating_sub(1));
        let k1 = (k0 + 1).min(grid.counts[2].saturating_sub(1));

        let at = |i, j, k| self.probes[grid.index(i, j, k)].sh;
        // Interpolate along x, then y, then z.
        let c00 = Sh9::lerp(&at(i0, j0, k0), &at(i1, j0, k0), frac.x);
        let c10 = Sh9::lerp(&at(i0, j1, k0), &at(i1, j1, k0), frac.x);
        let c01 = Sh9::lerp(&at(i0, j0, k1), &at(i1, j0, k1), frac.x);
        let c11 = Sh9::lerp(&at(i0, j1, k1), &at(i1, j1, k1), frac.x);
        let c0 = Sh9::lerp(&c00, &c10, frac.y);
        let c1 = Sh9::lerp(&c01, &c11, frac.y);
        Some(Sh9::lerp(&c0, &c1, frac.z))
    }

    /// The [`BLEND_PROBES`] nearest probes' SH, weighted by inverse squared
    /// distance (used for free probes or out-of-grid points). A point on a probe
    /// reads that probe; ties keep the lower index, so the answer is deterministic.
    fn sample_nearest(&self, position: Vec3) -> Sh9 {
        let index = self.index.get_or_init(|| ProbeIndex::build(&self.probes));
        let near = match index {
            Some(index) => index.nearest(&self.probes, position),
            None => index::scan(&self.probes, position),
        };
        self.blend(&near)
    }

    /// The first [`BLEND_PROBES`] of `near` blended by `1/d² − 1/d²ₙ`, where `dₙ`
    /// is the next probe's distance: a probe's weight falls to zero as the next one
    /// draws level and replaces it, so a swap never pops.
    fn blend(&self, near: &index::Nearest) -> Sh9 {
        let (blended, next) = near.split_at(BLEND_PROBES);
        let found = || blended.iter().filter(|n| n.1 != usize::MAX);
        if let Some(&(_, i)) = found().find(|n| n.0 <= ON_PROBE) {
            return self.probes[i].sh;
        }
        // `1/dₙ²` is 0 with no next probe (an infinite distance); the plain weights
        // are the fallback when every blended probe is as far as the next.
        for fade in [1.0 / next[0].0, 0.0] {
            let (mut sum, mut total) = (Sh9::zero(), 0.0);
            for &(d, i) in found() {
                let w = 1.0 / d - fade;
                if w > 0.0 {
                    sum = Sh9::lerp(&sum, &self.probes[i].sh, w / (total + w));
                    total += w;
                }
            }
            if total > 0.0 {
                return sum;
            }
        }
        Sh9::zero()
    }
}
