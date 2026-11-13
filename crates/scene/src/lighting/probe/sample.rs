//! crates/scene/src/lighting/probe/sample.rs — reading the probe field at a point:
//! trilinear over the grid, else the [`BLEND_PROBES`] nearest probes blended. The
//! SH and the shadowmask occlusion (#1695) blend with the same weights.

use glam::{Vec3, Vec4};

use super::index::{self, ProbeIndex};
use super::{Probe, ProbeGrid, ProbeVolume, BLEND_PROBES, ON_PROBE};
use crate::lighting::sh::Sh9;

/// A value a probe carries that the field interpolates.
trait Blend: Copy {
    /// What a blend with no weight gives.
    fn none() -> Self;
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
}

impl Blend for Sh9 {
    fn none() -> Self {
        Sh9::zero()
    }
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Sh9::lerp(a, b, t)
    }
}

impl Blend for Vec4 {
    /// No probe weighs in: nothing is known to occlude.
    fn none() -> Self {
        Vec4::ONE
    }
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a.lerp(*b, t)
    }
}

impl ProbeVolume {
    /// Sample the interpolated SH irradiance at a world position. Trilinear over the
    /// grid when one is present and the point falls inside it; otherwise (free probes,
    /// or a point outside the grid) the [`BLEND_PROBES`] nearest blended. `None` when
    /// there are no probes.
    pub fn sample(&self, position: Vec3) -> Option<Sh9> {
        self.sample_by(position, |p| p.sh)
    }

    /// The shadowmask occlusion at a world position (#1695): per channel, how much
    /// of its `Mixed` light the probes saw past static occluders, interpolated as
    /// [`Self::sample`] interpolates the SH. `None` when there are no probes.
    pub fn sample_occlusion(&self, position: Vec3) -> Option<Vec4> {
        self.sample_by(position, |p| p.occlusion)
    }

    fn sample_by<T: Blend>(&self, position: Vec3, get: impl Fn(&Probe) -> T) -> Option<T> {
        if self.probes.is_empty() {
            return None;
        }
        if let Some(grid) = &self.grid {
            if let Some(v) = self.sample_trilinear(grid, position, &get) {
                return Some(v);
            }
        }
        Some(self.sample_nearest(position, &get))
    }

    /// Trilinear interpolation within the grid; `None` if the point lies outside the
    /// lattice bounds (the caller then falls back to nearest).
    fn sample_trilinear<T: Blend>(
        &self,
        grid: &ProbeGrid,
        position: Vec3,
        get: &impl Fn(&Probe) -> T,
    ) -> Option<T> {
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

        let at = |i, j, k| get(&self.probes[grid.index(i, j, k)]);
        // Interpolate along x, then y, then z.
        let c00 = T::lerp(&at(i0, j0, k0), &at(i1, j0, k0), frac.x);
        let c10 = T::lerp(&at(i0, j1, k0), &at(i1, j1, k0), frac.x);
        let c01 = T::lerp(&at(i0, j0, k1), &at(i1, j0, k1), frac.x);
        let c11 = T::lerp(&at(i0, j1, k1), &at(i1, j1, k1), frac.x);
        let c0 = T::lerp(&c00, &c10, frac.y);
        let c1 = T::lerp(&c01, &c11, frac.y);
        Some(T::lerp(&c0, &c1, frac.z))
    }

    /// The [`BLEND_PROBES`] nearest probes' SH, weighted by inverse squared
    /// distance (used for free probes or out-of-grid points). A point on a probe
    /// reads that probe; ties keep the lower index, so the answer is deterministic.
    fn sample_nearest<T: Blend>(&self, position: Vec3, get: &impl Fn(&Probe) -> T) -> T {
        let index = self.index.get_or_init(|| ProbeIndex::build(&self.probes));
        let near = match index {
            Some(index) => index.nearest(&self.probes, position),
            None => index::scan(&self.probes, position),
        };
        self.blend(&near, get)
    }

    /// The first [`BLEND_PROBES`] of `near` blended by `1/d² − 1/d²ₙ`, where `dₙ`
    /// is the next probe's distance: a probe's weight falls to zero as the next one
    /// draws level and replaces it, so a swap never pops.
    fn blend<T: Blend>(&self, near: &index::Nearest, get: &impl Fn(&Probe) -> T) -> T {
        let (blended, next) = near.split_at(BLEND_PROBES);
        let found = || blended.iter().filter(|n| n.1 != usize::MAX);
        if let Some(&(_, i)) = found().find(|n| n.0 <= ON_PROBE) {
            return get(&self.probes[i]);
        }
        // `1/dₙ²` is 0 with no next probe (an infinite distance); the plain weights
        // are the fallback when every blended probe is as far as the next.
        for fade in [1.0 / next[0].0, 0.0] {
            let (mut sum, mut total) = (T::none(), 0.0);
            for &(d, i) in found() {
                let w = 1.0 / d - fade;
                if w > 0.0 {
                    sum = T::lerp(&sum, &get(&self.probes[i]), w / (total + w));
                    total += w;
                }
            }
            if total > 0.0 {
                return sum;
            }
        }
        T::none()
    }
}
