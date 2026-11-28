//! Light probes are a scene-level dataset, not a component. Written only through
//! `ProbeVolume`'s methods, which drop the stale index and move the revision the renderer
//! reads.

mod index;
mod sample;

use std::sync::OnceLock;

use glam::{Vec3, Vec4};
use serde::{Deserialize, Serialize};

use crate::lighting::sh::Sh9;
use crate::revision::Revision;
use index::ProbeIndex;

/// Probes a point outside any grid blends: four, as Unity's light-probe tetrahedra do.
pub const BLEND_PROBES: usize = 4;
/// Squared metres: a point this near a probe reads it unblended.
const ON_PROBE: f32 = 1.0e-8;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub position: Vec3,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub manual: bool,
    #[serde(skip)]
    pub sh: Sh9,
    /// Per shadowmask channel, how much `Mixed` light reaches it past static occluders: 1 lit.
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeGrid {
    pub origin: Vec3,
    pub spacing: Vec3,
    pub counts: [u32; 3],
}

impl ProbeGrid {
    pub fn probe_count(&self) -> usize {
        self.counts[0] as usize * self.counts[1] as usize * self.counts[2] as usize
    }

    fn index(&self, i: u32, j: u32, k: u32) -> usize {
        let (nx, ny) = (self.counts[0], self.counts[1]);
        (k * nx * ny + j * nx + i) as usize
    }

    fn world_pos(&self, i: u32, j: u32, k: u32) -> Vec3 {
        self.origin + self.spacing * Vec3::new(i as f32, j as f32, k as f32)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProbeVolume {
    probes: Vec<Probe>,
    #[serde(default)]
    grid: Option<ProbeGrid>,
    #[serde(skip)]
    index: OnceLock<Option<ProbeIndex>>,
    #[serde(skip)]
    revision: Revision,
    /// The floating origin's offset: a sample at a World position reads the authored one.
    #[serde(skip)]
    origin: Vec3,
}

impl ProbeVolume {
    pub fn set_origin(&mut self, offset: Vec3) {
        self.origin = offset;
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }

    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    pub fn probes_mut(&mut self) -> &mut [Probe] {
        self.reindex();
        &mut self.probes
    }

    pub fn grid(&self) -> Option<ProbeGrid> {
        self.grid
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn add_probe(&mut self, position: Vec3) -> usize {
        self.reindex();
        self.probes.push(Probe::new(position));
        self.probes.len() - 1
    }

    pub fn move_probe(&mut self, index: usize, position: Vec3) {
        if let Some(p) = self.probes.get_mut(index) {
            p.position = position;
            self.reindex();
        }
    }

    pub fn remove_probe(&mut self, index: usize) {
        if index < self.probes.len() {
            self.probes.remove(index);
            self.grid = None;
            self.reindex();
        }
    }

    pub fn set_manual(&mut self, index: usize, manual: bool) {
        if let Some(p) = self.probes.get_mut(index) {
            p.manual = manual;
            self.revision = Revision::default();
        }
    }

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

    pub fn clear(&mut self) {
        self.probes.clear();
        self.grid = None;
        self.reindex();
    }

    fn reindex(&mut self) {
        self.index = OnceLock::new();
        self.revision = Revision::default();
    }

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
