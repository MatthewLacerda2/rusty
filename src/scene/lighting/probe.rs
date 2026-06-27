//! src/scene/probe.rs — Light-probe data model + trilinear sampling.
//!
//! A light probe captures the bounced/ambient light arriving at a point as an L2
//! SH irradiance signal (see `sh.rs`). Probes are a SCENE-LEVEL DATASET, not a
//! first-class component: an array of probes lives on the scene, and dynamic
//! (non-static) objects sample the interpolated probe field instead of the flat
//! hemispherical ambient term.
//!
//! Storage split (mirrors the scene ↔ sidecar layout of `skybox_path`):
//!   * Probe POSITIONS + the grid layout live in the scene document (references +
//!     values, no GPU buffers).
//!   * The baked SH coefficients live in a SIDECAR `<scene>.lighting.json`
//!     (`LightingData`), keyed by probe index — heavy, regenerable data kept out
//!     of the human-diffable scene file.
//!
//! Interpolation is TRILINEAR over the grid (no tetrahedral gold-plating). A free
//! probe (placed outside any grid) contributes a nearest-probe fallback so manual
//! placement still works.
//!
//! This module is pure data + math — deterministic, sim-reachable, no RNG/clock.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::scene::lighting::sh::Sh9;

/// A single light probe: a world position plus the baked SH it carries. The SH is
/// the heavy part; on disk the position lives in the scene and the SH in the
/// sidecar (see [`ProbeVolume::split`]/[`ProbeVolume::merge`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub position: Vec3,
    #[serde(skip)]
    pub sh: Sh9,
}

impl Probe {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            sh: Sh9::zero(),
        }
    }
}

/// An optional regular grid describing how the probes are laid out. When present,
/// sampling uses trilinear interpolation over the lattice; when absent, probes are
/// sampled by nearest neighbour. `origin` is the min corner, `spacing` the cell
/// size, `counts` the probe count along each axis (so `count.x*count.y*count.z`
/// probes, ordered x-fastest then y then z).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeGrid {
    pub origin: Vec3,
    pub spacing: Vec3,
    pub counts: [u32; 3],
}

impl ProbeGrid {
    /// Total number of probes a grid of these dimensions holds.
    pub fn probe_count(&self) -> usize {
        self.counts[0] as usize * self.counts[1] as usize * self.counts[2] as usize
    }

    /// Linear index of the probe at lattice coordinate (i, j, k), x-fastest.
    fn index(&self, i: u32, j: u32, k: u32) -> usize {
        let (nx, ny) = (self.counts[0], self.counts[1]);
        (k * nx * ny + j * nx + i) as usize
    }

    /// World position of the probe at lattice coordinate (i, j, k).
    fn world_pos(&self, i: u32, j: u32, k: u32) -> Vec3 {
        self.origin + self.spacing * Vec3::new(i as f32, j as f32, k as f32)
    }
}

/// The scene-level probe dataset: the probe array plus an optional grid layout.
/// Owned by `Scene`. Save/load splits the SH out into the sidecar.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProbeVolume {
    pub probes: Vec<Probe>,
    #[serde(default)]
    pub grid: Option<ProbeGrid>,
}

impl ProbeVolume {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }

    /// Add a single free probe at `position`, returning its index.
    pub fn add_probe(&mut self, position: Vec3) -> usize {
        self.probes.push(Probe::new(position));
        self.probes.len() - 1
    }

    /// Move probe `index` to a new position (no-op if out of range).
    pub fn move_probe(&mut self, index: usize, position: Vec3) {
        if let Some(p) = self.probes.get_mut(index) {
            p.position = position;
        }
    }

    /// Remove probe `index` (no-op if out of range). Removing a probe invalidates the
    /// grid layout (indices shift), so the grid is dropped to keep the two consistent.
    pub fn remove_probe(&mut self, index: usize) {
        if index < self.probes.len() {
            self.probes.remove(index);
            self.grid = None;
        }
    }

    /// Drop every probe and the grid.
    pub fn clear(&mut self) {
        self.probes.clear();
        self.grid = None;
    }

    /// Replace the probe set with a regular grid spanning `[min, max]` at `spacing`
    /// world units. At least one probe per axis. Existing probes are discarded. The
    /// SH stays zero until a fill/bake runs.
    pub fn fill_grid(&mut self, min: Vec3, max: Vec3, spacing: f32) {
        let spacing = spacing.max(1.0e-3);
        let span = (max - min).max(Vec3::ZERO);
        let counts = [
            (span.x / spacing).floor() as u32 + 1,
            (span.y / spacing).floor() as u32 + 1,
            (span.z / spacing).floor() as u32 + 1,
        ];
        let grid = ProbeGrid {
            origin: min,
            spacing: Vec3::splat(spacing),
            counts,
        };
        self.probes = Vec::with_capacity(grid.probe_count());
        for k in 0..counts[2] {
            for j in 0..counts[1] {
                for i in 0..counts[0] {
                    self.probes.push(Probe::new(grid.world_pos(i, j, k)));
                }
            }
        }
        self.grid = Some(grid);
    }

    /// Sample the interpolated SH irradiance at a world position. Trilinear over the
    /// grid when one is present and the point falls inside it; otherwise (free probes,
    /// or a point outside the grid) the nearest probe. `None` when there are no probes.
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

    /// Nearest-probe SH (used for free probes or out-of-grid points).
    fn sample_nearest(&self, position: Vec3) -> Sh9 {
        let mut best = (f32::INFINITY, Sh9::zero());
        for p in &self.probes {
            let d = p.position.distance_squared(position);
            if d < best.0 {
                best = (d, p.sh);
            }
        }
        best.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_grid_counts_and_positions() {
        let mut vol = ProbeVolume::new();
        vol.fill_grid(Vec3::ZERO, Vec3::new(2.0, 0.0, 2.0), 1.0);
        // 3 along x, 1 along y, 3 along z = 9 probes.
        assert_eq!(vol.probes.len(), 9);
        let grid = vol.grid.unwrap();
        assert_eq!(grid.counts, [3, 1, 3]);
        assert_eq!(vol.probes[0].position, Vec3::ZERO);
    }

    #[test]
    fn add_move_remove_probe() {
        let mut vol = ProbeVolume::new();
        let i = vol.add_probe(Vec3::X);
        assert_eq!(vol.probes.len(), 1);
        vol.move_probe(i, Vec3::Y);
        assert_eq!(vol.probes[i].position, Vec3::Y);
        vol.remove_probe(i);
        assert!(vol.is_empty());
    }

    #[test]
    fn sample_none_when_empty() {
        let vol = ProbeVolume::new();
        assert!(vol.sample(Vec3::ZERO).is_none());
    }

    #[test]
    fn trilinear_blends_between_probes() {
        let mut vol = ProbeVolume::new();
        vol.fill_grid(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0);
        // Two probes along x. Set distinct DC terms.
        vol.probes[0].sh.coeffs[0] = [0.0, 0.0, 0.0];
        vol.probes[1].sh.coeffs[0] = [1.0, 0.0, 0.0];
        let mid = vol.sample(Vec3::new(0.5, 0.0, 0.0)).unwrap();
        assert!((mid.coeffs[0][0] - 0.5).abs() < 1.0e-5);
    }
}
