//! The scene-level reflection-probe dataset: the probes in placement order, and the
//! edits the editor and `Reflection.*` make to them, addressed by index.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::ReflectionProbe;

/// The scene-level reflection-probe dataset. Owned by `Scene`, serialized into the
/// scene document (positions, boxes, settings and cubemap paths; no GPU data).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReflectionProbeSet {
    pub probes: Vec<ReflectionProbe>,
}

impl ReflectionProbeSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.probes.len()
    }

    /// Add a probe at `position` with a box of the given half-extents, returning its index.
    pub fn add(&mut self, position: Vec3, half_extents: Vec3) -> usize {
        self.probes
            .push(ReflectionProbe::new(position, half_extents));
        self.probes.len() - 1
    }

    /// Move probe `index` to `position`, shifting its box to stay centred (no-op if out
    /// of range).
    pub fn move_probe(&mut self, index: usize, position: Vec3) {
        if let Some(p) = self.probes.get_mut(index) {
            let he = (p.box_max - p.box_min) * 0.5;
            p.position = position;
            p.box_min = position - he;
            p.box_max = position + he;
        }
    }

    /// Set probe `index`'s parallax box to the explicit `[min, max]` corners (no-op if
    /// out of range). Corners are normalized so `box_min <= box_max` componentwise.
    pub fn set_box(&mut self, index: usize, min: Vec3, max: Vec3) {
        if let Some(p) = self.probes.get_mut(index) {
            p.box_min = min.min(max);
            p.box_max = min.max(max);
        }
    }

    /// Point probe `index` at a baked cubemap path (no-op if out of range).
    pub fn set_cubemap(&mut self, index: usize, path: String) {
        if let Some(p) = self.probes.get_mut(index) {
            p.cubemap_path = path;
        }
    }

    /// Edit probe `index` in place, then hold its settings to their ranges and its
    /// box corners in order: the one path the Inspector and `Reflection.Set` write
    /// through. `false` when there is no such probe.
    pub fn edit(&mut self, index: usize, f: impl FnOnce(&mut ReflectionProbe)) -> bool {
        let Some(p) = self.probes.get_mut(index) else {
            return false;
        };
        f(p);
        p.clamp_settings();
        (p.box_min, p.box_max) = (p.box_min.min(p.box_max), p.box_min.max(p.box_max));
        true
    }

    /// Remove probe `index` (no-op if out of range).
    pub fn remove(&mut self, index: usize) {
        if index < self.probes.len() {
            self.probes.remove(index);
        }
    }

    /// Drop every probe.
    pub fn clear(&mut self) {
        self.probes.clear();
    }
}
