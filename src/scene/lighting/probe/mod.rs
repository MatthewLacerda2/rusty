//! src/scene/lighting/probe/ — Light-probe data model + trilinear sampling.
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
//!   * The baked SH coefficients, and a `Shadowmask` bake's per-probe occlusion
//!     (#1695), live in a SIDECAR `<scene>.lighting.json` (`LightingData`), keyed by
//!     probe index — heavy, regenerable data kept out of the human-diffable scene
//!     file. The occlusion blends with the SH's weights (`sample_occlusion`).
//!
//! Interpolation is TRILINEAR over the grid (no tetrahedral gold-plating). Outside
//! a grid, or for a set with none (the automatic placement's, #1737, whose density
//! follows the geometry), the [`BLEND_PROBES`] nearest probes are blended by inverse
//! squared distance less the next one's, so a probe's weight reaches zero just as it
//! leaves the blend and a moving object fades from one probe to the next. They are
//! found through a spatial index (`index.rs`) built when the set is first sampled
//! after a change, so a sample reads the probes near it, not the whole set.
//!
//! The probes and the grid are written only through [`ProbeVolume`]'s methods
//! ([`ProbeVolume::probes_mut`] for the baked SH and a probe's own fields), which
//! drop the stale index and move the set's [`ProbeVolume::revision`]: how the
//! renderer learns the field changed without hashing it every frame
//! (`scene::revision`, #1706).
//!
//! This module is pure data + math — deterministic, sim-reachable, no RNG/clock.

mod index;
mod sample;

use std::sync::OnceLock;

use glam::{Vec3, Vec4};
use serde::{Deserialize, Serialize};

use crate::scene::lighting::sh::Sh9;
use crate::scene::revision::Revision;
use index::ProbeIndex;

/// How many probes a point outside any grid blends: the four nearest, a
/// tetrahedron's worth, as Unity's light-probe tetrahedra blend four.
pub const BLEND_PROBES: usize = 4;
/// A squared distance this small reads the probe it is on, unblended.
const ON_PROBE: f32 = 1.0e-8;

/// A single light probe: a world position plus the baked SH it carries. The SH is
/// the heavy part; on disk the position lives in the scene and the SH in the
/// sidecar (see [`save_lighting_sidecar`](crate::scene::lighting::io::save_lighting_sidecar)/
/// [`load_lighting_sidecar`](crate::scene::lighting::io::load_lighting_sidecar)).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub position: Vec3,
    /// Placed by hand (#1737): the automatic placement keeps it where it is and
    /// plans around it, and replaces only the probes without it. `Probe.Add` and
    /// `Probe.FillGrid` set it; a saved probe without the field reads as placed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub manual: bool,
    #[serde(skip)]
    pub sh: Sh9,
    /// Per shadowmask channel (#1695), how much of its `Mixed` light reaches the
    /// probe past static occluders: 1 lit, 0 shadowed (Unity's light-probe
    /// occlusion). All ones until a `Shadowmask` bake writes it; in the sidecar
    /// beside the SH.
    #[serde(skip, default = "unoccluded")]
    pub occlusion: Vec4,
}

fn unoccluded() -> Vec4 {
    Vec4::ONE
}

impl Probe {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            manual: false,
            sh: Sh9::zero(),
            occlusion: Vec4::ONE,
        }
    }
}

/// An optional regular grid describing how the probes are laid out. When present,
/// sampling uses trilinear interpolation over the lattice; when absent, the
/// [`BLEND_PROBES`] nearest probes are blended. `origin` is the min corner, `spacing` the cell
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
    probes: Vec<Probe>,
    #[serde(default)]
    grid: Option<ProbeGrid>,
    /// The nearest-probe index, built on the first sample after a change.
    #[serde(skip)]
    index: OnceLock<Option<ProbeIndex>>,
    #[serde(skip)]
    revision: Revision,
}

impl ProbeVolume {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }

    /// The probes, in index order.
    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    /// The probes to write in place (the baked SH, a position, the manual flag): the
    /// set counts as changed.
    pub fn probes_mut(&mut self) -> &mut [Probe] {
        self.reindex();
        &mut self.probes
    }

    /// The grid layout, if the probes form one.
    pub fn grid(&self) -> Option<ProbeGrid> {
        self.grid
    }

    /// What the set was last written as: equal across two reads exactly when no
    /// write came between them, or one set is a copy of the other. The renderer's
    /// retained objects hold probe SH and read this to know when to re-sample.
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Add a single free probe at `position`, returning its index.
    pub fn add_probe(&mut self, position: Vec3) -> usize {
        self.reindex();
        self.probes.push(Probe::new(position));
        self.probes.len() - 1
    }

    /// Move probe `index` to a new position (no-op if out of range).
    pub fn move_probe(&mut self, index: usize, position: Vec3) {
        if let Some(p) = self.probes.get_mut(index) {
            p.position = position;
            self.reindex();
        }
    }

    /// Remove probe `index` (no-op if out of range). Removing a probe invalidates the
    /// grid layout (indices shift), so the grid is dropped to keep the two consistent.
    pub fn remove_probe(&mut self, index: usize) {
        if index < self.probes.len() {
            self.probes.remove(index);
            self.grid = None;
            self.reindex();
        }
    }

    /// Mark probe `index` as placed by hand or not (no-op if out of range).
    pub fn set_manual(&mut self, index: usize, manual: bool) {
        if let Some(p) = self.probes.get_mut(index) {
            p.manual = manual;
            self.revision = Revision::default();
        }
    }

    /// Replace the probes the automatic placement made with `placed` (#1737): the
    /// manual ones stay, in order, and the new ones follow them. The grid stays
    /// when every probe it indexes (the first [`ProbeGrid::probe_count`]) is manual,
    /// since they keep their indices; placed probes after it never shift them.
    pub fn replace_placed(&mut self, placed: impl IntoIterator<Item = Vec3>) {
        let gridded = self.grid.map_or(0, |g| g.probe_count());
        let grid_manual =
            gridded <= self.probes.len() && self.probes[..gridded].iter().all(|p| p.manual);
        if !grid_manual {
            self.grid = None;
        }
        self.probes.retain(|p| p.manual);
        self.probes.extend(placed.into_iter().map(Probe::new));
        self.reindex();
    }

    /// Drop every probe and the grid.
    pub fn clear(&mut self) {
        self.probes.clear();
        self.grid = None;
        self.reindex();
    }

    /// Drop the nearest-probe index and take a fresh revision: the set changed.
    fn reindex(&mut self) {
        self.index = OnceLock::new();
        self.revision = Revision::default();
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
        self.reindex();
    }
}

#[cfg(test)]
mod blend_tests;
#[cfg(test)]
mod tests;
