use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::ReflectionProbe;
use crate::lighting::sh::Sh9;

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

    pub fn add(&mut self, position: Vec3, half_extents: Vec3) -> usize {
        self.probes
            .push(ReflectionProbe::new(position, half_extents));
        self.probes.len() - 1
    }

    pub fn move_probe(&mut self, index: usize, position: Vec3) {
        if let Some(p) = self.probes.get_mut(index) {
            let he = (p.box_max - p.box_min) * 0.5;
            p.position = position;
            p.box_min = position - he;
            p.box_max = position + he;
        }
    }

    pub fn set_box(&mut self, index: usize, min: Vec3, max: Vec3) {
        if let Some(p) = self.probes.get_mut(index) {
            p.box_min = min.min(max);
            p.box_max = min.max(max);
        }
    }

    /// A new cube drops the old one's irradiance: it is not normalised by another cube's light.
    pub fn set_cubemap(&mut self, index: usize, path: String) {
        if let Some(p) = self.probes.get_mut(index) {
            if p.cubemap_path != path {
                p.irradiance = None;
            }
            p.cubemap_path = path;
        }
    }

    /// The one path the Inspector and `Reflection.Set` write through.
    pub fn edit(&mut self, index: usize, f: impl FnOnce(&mut ReflectionProbe)) -> bool {
        let Some(p) = self.probes.get_mut(index) else {
            return false;
        };
        f(p);
        hold(p);
        true
    }

    /// A loaded file goes through this once, so no reader clamps again; of several fallbacks
    /// the first stays.
    pub fn hold(&mut self) {
        self.probes.iter_mut().for_each(hold);
        self.set_fallback(self.fallback());
    }

    pub fn fallback(&self) -> Option<usize> {
        self.probes.iter().position(|p| p.fallback)
    }

    pub fn set_fallback(&mut self, index: Option<usize>) -> bool {
        if index.is_some_and(|i| i >= self.probes.len()) {
            return false;
        }
        for (i, p) in self.probes.iter_mut().enumerate() {
            p.fallback = Some(i) == index;
            p.manual |= p.fallback;
        }
        true
    }

    pub fn set_irradiance(&mut self, index: usize, irradiance: Option<Sh9>) {
        if let Some(p) = self.probes.get_mut(index) {
            p.irradiance = irradiance;
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.probes.len() {
            self.probes.remove(index);
        }
    }

    pub fn clear(&mut self) {
        self.probes.clear();
    }
}

/// A non-finite stored irradiance (a damaged file) goes, so the probe scales by 1, not NaN.
/// The fallback stays manual, so a bake's placement never deletes it.
fn hold(p: &mut ReflectionProbe) {
    p.clamp_settings();
    p.manual |= p.fallback;
    if p.irradiance
        .is_some_and(|sh| sh.coeffs.as_flattened().iter().any(|c| !c.is_finite()))
    {
        p.irradiance = None;
    }
    (p.box_min, p.box_max) = (p.box_min.min(p.box_max), p.box_min.max(p.box_max));
}
