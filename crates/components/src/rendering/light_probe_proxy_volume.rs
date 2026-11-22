//! Unity's `LightProbeProxyVolume`. Pure authoring data: the grid's samples are never saved; the
//! renderer takes them from the probe field each frame (`render::draw::proxy_volume`).

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const MAX_RESOLUTION: u32 = 32;
/// Grid points per metre.
pub const DENSITY_RANGE: (f32, f32) = (0.1, 100.0);
/// Metres: a flat mesh still gets a one-point-thick grid.
const MIN_SIZE: f32 = 1e-3;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ProxyBoundsMode {
        #[default]
        AutomaticLocal = "AutomaticLocal" | "Automatic",
        Custom = "Custom",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ProxyResolutionMode {
        #[default]
        Automatic = "Automatic",
        Custom = "Custom",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ProxyQuality {
        Low = "Low" | "L0",
        #[default]
        Normal = "Normal" | "L1",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightProbeProxyVolumeComponent {
    pub bounds_mode: ProxyBoundsMode,
    pub size: Vec3,
    pub origin: Vec3,
    pub resolution_mode: ProxyResolutionMode,
    pub probe_density: f32,
    pub grid_resolution: [u32; 3],
    pub quality: ProxyQuality,
}

impl Default for LightProbeProxyVolumeComponent {
    fn default() -> Self {
        Self {
            bounds_mode: ProxyBoundsMode::AutomaticLocal,
            size: Vec3::ONE,
            origin: Vec3::ZERO,
            resolution_mode: ProxyResolutionMode::Automatic,
            probe_density: 1.0,
            grid_resolution: [4, 4, 4],
            quality: ProxyQuality::Normal,
        }
    }
}

/// `dims` points per axis, each at the centre of its cell of `[min, min + size]`; x fastest, then
/// y, then z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProxyGrid {
    pub min: Vec3,
    pub size: Vec3,
    pub dims: [u32; 3],
}

impl ProxyGrid {
    pub fn len(&self) -> usize {
        self.dims.iter().map(|&n| n as usize).product()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn point(&self, i: u32, j: u32, k: u32) -> Vec3 {
        let cell = |n: u32, d: u32| (n as f32 + 0.5) / d as f32;
        let [x, y, z] = self.dims;
        self.min + self.size * Vec3::new(cell(i, x), cell(j, y), cell(k, z))
    }

    pub fn points(&self) -> impl Iterator<Item = Vec3> + '_ {
        let [x, y, z] = self.dims;
        (0..z).flat_map(move |k| (0..y).flat_map(move |j| (0..x).map(move |i| self.point(i, j, k))))
    }
}

impl LightProbeProxyVolumeComponent {
    /// A non-finite box or density falls back to the defaults, so a malformed file still draws.
    pub fn grid(&self, mesh_bounds: (Vec3, Vec3)) -> ProxyGrid {
        let (centre, size) = match self.bounds_mode {
            ProxyBoundsMode::AutomaticLocal => {
                let (min, max) = mesh_bounds;
                ((min + max) * 0.5, max - min)
            }
            ProxyBoundsMode::Custom => (self.origin, self.size),
        };
        let fine = |v: Vec3, or: Vec3| if v.is_finite() { v } else { or };
        let size = fine(size, Vec3::ONE).abs().max(Vec3::splat(MIN_SIZE));
        let centre = fine(centre, Vec3::ZERO);
        let dims = match self.resolution_mode {
            ProxyResolutionMode::Automatic => {
                let d = clamp_density(self.probe_density);
                [size.x, size.y, size.z].map(|s| resolution((s * d).round() as u32))
            }
            ProxyResolutionMode::Custom => self.grid_resolution.map(resolution),
        };
        ProxyGrid {
            min: centre - size * 0.5,
            size,
            dims,
        }
    }
}

pub fn resolution(n: u32) -> u32 {
    n.clamp(1, MAX_RESOLUTION).next_power_of_two()
}

/// Clamped to [`DENSITY_RANGE`]; NaN reads as `1.0`.
pub fn clamp_density(d: f32) -> f32 {
    if d.is_nan() {
        1.0
    } else {
        d.clamp(DENSITY_RANGE.0, DENSITY_RANGE.1)
    }
}

impl crate::FirstClass for LightProbeProxyVolumeComponent {
    const NAME: &'static str = "LightProbeProxyVolume";
    const ALIASES: &'static [&'static str] = &["proxyvolume", "lppv"];
}

#[cfg(test)]
#[path = "light_probe_proxy_volume_tests.rs"]
mod tests;
