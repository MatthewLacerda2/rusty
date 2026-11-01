//! src/components/rendering/light_probe_proxy_volume.rs — Light Probe Proxy Volume
//! (#1672).
//!
//! Unity's `LightProbeProxyVolume`: a mesh entity that carries one is lit from the
//! light probes sampled on a small 3D grid over its bounds, interpolated per pixel,
//! instead of from one sample at its origin. A 2 m locker under a wall lamp then
//! goes from a lit top to a dark foot; without the component it stays one colour.
//!
//! Pure authoring data, values only: the box (the mesh's own local bounds, or a
//! custom size and origin in the entity's local space), the grid resolution (a
//! density per metre, or a count per axis, each a power of two from 1 to 32, as
//! Unity caps it) and the SH quality (L1, or L0 for one flat colour per point).
//! The grid's samples are never saved: the renderer takes them from the probe field
//! each frame the entity, its settings or the probes change
//! (`render::draw::proxy_volume`), Unity's *Automatic* refresh.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// The most grid points along one axis (Unity's cap).
pub const MAX_RESOLUTION: u32 = 32;
/// The density range, in grid points per metre.
pub const DENSITY_RANGE: (f32, f32) = (0.1, 100.0);
/// The thinnest box side the grid spans, in metres: a flat mesh still gets a
/// one-point-thick grid.
const MIN_SIZE: f32 = 1e-3;

crate::core::named_enum! {
    /// Where the volume's box comes from (Unity's `BoundingBoxMode`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ProxyBoundsMode {
        /// The entity's mesh bounds, in its local space.
        #[default]
        AutomaticLocal = "AutomaticLocal" | "Automatic",
        /// `size` and `origin` (its centre), in the entity's local space.
        Custom = "Custom",
    }
}

crate::core::named_enum! {
    /// How many grid points the box gets (Unity's `ResolutionMode`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ProxyResolutionMode {
        /// `probe_density` points per metre along each axis.
        #[default]
        Automatic = "Automatic",
        /// `grid_resolution` points per axis.
        Custom = "Custom",
    }
}

crate::core::named_enum! {
    /// The SH bands each grid point keeps (Unity's `QualityMode`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ProxyQuality {
        /// L0: one colour per point, the same from every direction.
        Low = "Low" | "L0",
        /// L1: a colour plus its direction per point.
        #[default]
        Normal = "Normal" | "L1",
    }
}

/// An entity's Light Probe Proxy Volume.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightProbeProxyVolumeComponent {
    pub bounds_mode: ProxyBoundsMode,
    /// The box's size under `Custom` bounds, in local metres.
    pub size: Vec3,
    /// The box's centre under `Custom` bounds, in local space.
    pub origin: Vec3,
    pub resolution_mode: ProxyResolutionMode,
    /// Grid points per metre under `Automatic` resolution.
    pub probe_density: f32,
    /// Grid points per axis under `Custom` resolution, each a power of two.
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

/// The grid a volume resolves to, in the entity's local space: `dims` points per
/// axis, each at the centre of its cell of the box `[min, min + size]` (a 3D
/// texture's texel centres), x fastest, then y, then z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProxyGrid {
    pub min: Vec3,
    pub size: Vec3,
    pub dims: [u32; 3],
}

impl ProxyGrid {
    /// How many points the grid holds.
    pub fn len(&self) -> usize {
        self.dims.iter().map(|&n| n as usize).product()
    }

    /// Whether the grid holds no point (never, for a resolved grid).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The local position of point `(i, j, k)`.
    pub fn point(&self, i: u32, j: u32, k: u32) -> Vec3 {
        let cell = |n: u32, d: u32| (n as f32 + 0.5) / d as f32;
        let [x, y, z] = self.dims;
        self.min + self.size * Vec3::new(cell(i, x), cell(j, y), cell(k, z))
    }

    /// Every point's local position, in storage order.
    pub fn points(&self) -> impl Iterator<Item = Vec3> + '_ {
        let [x, y, z] = self.dims;
        (0..z).flat_map(move |k| (0..y).flat_map(move |j| (0..x).map(move |i| self.point(i, j, k))))
    }
}

impl LightProbeProxyVolumeComponent {
    /// The grid this volume spans over a mesh whose local bounds are `mesh_bounds`
    /// (`(min, max)`): the box its mode picks, at its resolution. A non-finite box or
    /// density falls back to the defaults', so a malformed file still draws.
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

/// `n` as a grid resolution: the next power of two, from 1 to [`MAX_RESOLUTION`].
pub fn resolution(n: u32) -> u32 {
    n.clamp(1, MAX_RESOLUTION).next_power_of_two()
}

/// `d` clamped to [`DENSITY_RANGE`]; NaN reads as the default `1.0`.
pub fn clamp_density(d: f32) -> f32 {
    if d.is_nan() {
        1.0
    } else {
        d.clamp(DENSITY_RANGE.0, DENSITY_RANGE.1)
    }
}

#[cfg(test)]
#[path = "light_probe_proxy_volume_tests.rs"]
mod tests;
