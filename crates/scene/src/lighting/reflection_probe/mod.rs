mod blend;
mod inspected;
mod projection;
mod set;
mod volume;

use std::ops::RangeInclusive;

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use super::sh::Sh9;
pub use blend::{PreparedProbes, ProbeBlend};
pub use set::ReflectionProbeSet;
pub use volume::ProbeFrame;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeShape {
    #[default]
    Box,
    Sphere,
}

impl ProbeShape {
    pub fn named(name: &str) -> Option<Self> {
        [Self::Box, Self::Sphere]
            .into_iter()
            .find(|s| format!("{s:?}") == name)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflectionProbe {
    pub position: Vec3,
    pub box_min: Vec3,
    pub box_max: Vec3,
    #[serde(skip_serializing_if = "is_identity")]
    pub rotation: Quat,
    #[serde(skip_serializing_if = "is_box")]
    pub shape: ProbeShape,
    #[serde(skip_serializing_if = "is_default_radius")]
    pub radius: f32,
    pub cubemap_path: String,
    pub box_projection: bool,
    pub blend_distance: f32,
    pub importance: i32,
    /// Face size in texels; `None` bakes at `LightingSettings::reflections`.
    pub resolution: Option<u32>,
    // The capture camera's clip planes, world units.
    pub near_clip: f32,
    pub far_clip: f32,
    /// Whether the capture renders shadows (Unity's *Shadow Distance* > 0).
    pub shadows: bool,
    pub intensity: f32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub manual: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irradiance: Option<Sh9>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fallback: bool,
}

impl Default for ReflectionProbe {
    /// Unity's new Reflection Probe: a 10 m box at the origin.
    fn default() -> Self {
        Self::new(Vec3::ZERO, Vec3::splat(5.0))
    }
}

impl ReflectionProbe {
    pub const RESOLUTION: RangeInclusive<u32> = 16..=2048;
    /// Metres: keeps the capture's near plane off zero.
    pub const MIN_NEAR_CLIP: f32 = 0.01;
    pub const INTENSITY: RangeInclusive<f32> = 0.0..=8.0;

    pub fn new(position: Vec3, half_extents: Vec3) -> Self {
        let he = half_extents.abs();
        Self {
            position,
            box_min: position - he,
            box_max: position + he,
            rotation: Quat::IDENTITY,
            shape: ProbeShape::Box,
            radius: DEFAULT_RADIUS,
            cubemap_path: String::new(),
            box_projection: true,
            blend_distance: 1.0,
            importance: 1,
            resolution: None,
            near_clip: 0.05,
            far_clip: 1000.0,
            shadows: true,
            intensity: 1.0,
            manual: false,
            irradiance: None,
            fallback: false,
        }
    }

    pub fn clamp_settings(&mut self) {
        // A non-finite value takes `new`'s default.
        self.resolution = self.resolution.map(clamp_resolution);
        self.blend_distance = finite_or(self.blend_distance, 1.0).max(0.0);
        self.near_clip = finite_or(self.near_clip, 0.05).max(Self::MIN_NEAR_CLIP);
        self.far_clip = finite_or(self.far_clip, 1000.0).max(self.near_clip * 2.0);
        let (lo, hi) = (*Self::INTENSITY.start(), *Self::INTENSITY.end());
        self.intensity = finite_or(self.intensity, 1.0).clamp(lo, hi);
        self.radius = finite_or(self.radius, DEFAULT_RADIUS).max(MIN_RADIUS);
        self.rotation = match self.rotation.is_finite() && self.rotation.length() > 1e-6 {
            true => self.rotation.normalize(),
            false => Quat::IDENTITY,
        };
    }
}

pub fn clamp_resolution(size: u32) -> u32 {
    let (lo, hi) = (
        *ReflectionProbe::RESOLUTION.start(),
        *ReflectionProbe::RESOLUTION.end(),
    );
    let size = size.clamp(lo, hi);
    let up = size.next_power_of_two();
    // The nearer power: 300 → 256, 400 → 512.
    match up - size <= size - up / 2 {
        true => up.min(hi),
        false => up / 2,
    }
}

/// Metres: the `Default` box's half-extent, as a sphere.
pub const DEFAULT_RADIUS: f32 = 5.0;
/// Metres: the floor on a radius and on the fade span `past_influence` divides by.
pub(super) const MIN_RADIUS: f32 = 0.01;

fn is_identity(q: &Quat) -> bool {
    *q == Quat::IDENTITY
}

fn is_box(shape: &ProbeShape) -> bool {
    *shape == ProbeShape::Box
}

fn is_default_radius(r: &f32) -> bool {
    *r == DEFAULT_RADIUS
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// Both mirrored by the shader's `parallax_correct`; keep them equal (WGSL may assume no inf
/// or NaN).
pub const PARALLEL_EPSILON: f32 = 1e-6;
pub const NO_HIT: f32 = 1e30;

#[cfg(test)]
#[path = "projection_far_tests.rs"]
mod projection_far_tests;
#[cfg(test)]
#[path = "projection_tests.rs"]
mod projection_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "volume_tests.rs"]
mod volume_tests;
