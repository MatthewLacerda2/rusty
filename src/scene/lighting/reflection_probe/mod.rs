//! src/scene/lighting/reflection_probe/ — Reflection-probe data model + parallax math.
//!
//! A reflection probe captures the surrounding environment as a prefiltered cubemap
//! so glossy surfaces reflect their actual room instead of the single global skybox.
//! Like light probes (#240), reflection probes are a SCENE-LEVEL DATASET, not a
//! first-class component: an array of probes lives on the scene. Each probe carries
//! Unity's Reflection Probe settings (Baked mode, #1584): box projection, blend
//! distance, importance, resolution, the capture's clip planes and shadows, and an
//! intensity. Like Unity's *Blend Probes*, the renderer picks up to two probes per
//! DRAWN OBJECT at its world-bounds centre and fades between them
//! ([`ReflectionProbeSet::blend`], `blend.rs`), so crossing a doorway fades instead
//! of popping.
//!
//! This is the SIBLING of the light-probe dataset, kept deliberately DISTINCT:
//!   * light probes (`probe.rs`) carry baked L2 SH irradiance for DIFFUSE ambient;
//!   * reflection probes (here) carry a prefiltered cubemap for SPECULAR reflection.
//!
//! Storage split (mirrors `skybox_path`): each probe stores a world POSITION, a
//! VOLUME (the parallax-correction and influence bounds, #1995: a box `box_min`/
//! `box_max` turned by `rotation` about the position, or a sphere of `radius` about
//! it; `volume.rs`), its settings, and a PATH to its baked cubemap sidecar (a KTX2
//! file). No GPU data lives
//! in the scene document. `Probe.Bake()` writes those cubemaps (#245); until a probe
//! is baked it may reference a file that doesn't exist yet, and the objects it would
//! cover reflect the skybox.
//!
//! The box-projected parallax correction (the standard Lagarde technique, HDRP's
//! oriented proxy) is a pure function (`projection.rs`) so it is unit-testable without
//! a GPU; `parallax_correct` in `engine/shaders/lighting/reflection_probe.wgsl` runs
//! the same steps with the same guards (no division by a zero component, no hit at or
//! past [`NO_HIT`]), since WGSL may assume no inf or NaN.
//!
//! This module is pure data + math — deterministic, no RNG/clock.

mod blend;
mod projection;
mod set;
mod volume;

use std::ops::RangeInclusive;

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use super::sh::Sh9;
pub use blend::ProbeBlend;
pub use set::ReflectionProbeSet;
pub use volume::ProbeFrame;

/// The shape of a probe's volume (#1995, HDRP's and Unreal's two): what its
/// reflection is projected against and where it applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeShape {
    /// The box `box_min`..`box_max`, turned by the probe's `rotation`.
    #[default]
    Box,
    /// A sphere of the probe's `radius` about its capture point.
    Sphere,
}

impl ProbeShape {
    /// The shape `Reflection.Set` names: `"Box"` or `"Sphere"`.
    pub fn named(name: &str) -> Option<Self> {
        [Self::Box, Self::Sphere]
            .into_iter()
            .find(|s| format!("{s:?}") == name)
    }
}

/// A single reflection probe: a capture position, the box volume used for parallax
/// correction and blending, Unity's per-probe settings, and the path to its baked
/// cubemap. The cubemap is a referenced sidecar (KTX2) — never inlined into the scene
/// document, mirroring `skybox_path`. A field the document omits takes its default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflectionProbe {
    /// World-space capture point — the cubemap's origin and the parallax pivot.
    pub position: Vec3,
    /// Min corner of the parallax-correction box: world space before
    /// [`Self::rotation`] turns the box about [`Self::position`].
    pub box_min: Vec3,
    /// Max corner of the parallax-correction box, as [`Self::box_min`].
    pub box_max: Vec3,
    /// The box's orientation (#1995, HDRP's oriented box): it turns about the
    /// capture point, for projection and influence alike. Identity is the
    /// world-aligned box. Probes are scene data, not entities, so there is no scale:
    /// the box's corners carry its size.
    #[serde(skip_serializing_if = "is_identity")]
    pub rotation: Quat,
    /// The volume's shape (#1995): the box, or a sphere of [`Self::radius`].
    #[serde(skip_serializing_if = "is_box")]
    pub shape: ProbeShape,
    /// The sphere's radius, world units (only read with [`ProbeShape::Sphere`]).
    #[serde(skip_serializing_if = "is_default_radius")]
    pub radius: f32,
    /// Path to the baked cubemap (a KTX2 file). May be empty / point at a not-yet-baked
    /// file (#245); the runtime then falls back to the global skybox.
    pub cubemap_path: String,
    /// Unity's *Box Projection*: reflections are parallax-corrected against the
    /// volume, so they track the room, also for a fragment outside it (#1995). Off
    /// reflects the cube as if infinitely far away.
    pub box_projection: bool,
    /// Unity's *Blend Distance*, world units: how far past its volume the probe
    /// still reaches, fading to nothing, so an object between two probes blends
    /// them. Influence is the volume grown by it; projection is the volume itself.
    pub blend_distance: f32,
    /// Unity's *Importance*: where boxes overlap, a higher one is drawn over a lower
    /// one; equal ones blend by how deep the object sits in each.
    pub importance: i32,
    /// The cubemap's face size in texels, a power of two in [`Self::RESOLUTION`];
    /// `None` bakes at the scene's default (`LightingSettings::reflections`).
    pub resolution: Option<u32>,
    /// The capture camera's near clip plane, world units.
    pub near_clip: f32,
    /// The capture camera's far clip plane, world units.
    pub far_clip: f32,
    /// Whether the capture renders shadows (Unity's *Shadow Distance* > 0).
    pub shadows: bool,
    /// Brightness of the reflection this probe gives, a multiplier on its cube.
    pub intensity: f32,
    /// Placed by hand (#1737): the automatic placement keeps it as it is (position,
    /// box and settings) and leaves the room it covers to it, replacing only the
    /// probes without it. `Reflection.Add` and the Inspector's Add set it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub manual: bool,
    /// The irradiance at the probe (#1697), stored by the bake beside
    /// [`Self::cubemap_path`]: its capture projected as a light probe's, the
    /// ambient where the view escapes (`reflection_bake::stored_irradiance`). The
    /// forward pass scales the probe's reflection at a pixel by the baked diffuse
    /// there over this along the normal, held to [0, 1] (probe normalisation);
    /// `None` (not baked, baked before it was stored, or pointed at another cube
    /// since) scales by 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irradiance: Option<Sh9>,
    /// Unity's default reflection probe (#1697): what an object no probe box
    /// reaches reflects instead of the sky, and what a partial blend leaves. At
    /// most one probe of a set has it ([`ReflectionProbeSet::set_fallback`]).
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
    /// The face sizes a probe may bake at: Unity's 16 to 2048.
    pub const RESOLUTION: RangeInclusive<u32> = 16..=2048;
    /// The shortest near clip a write accepts.
    pub const MIN_NEAR_CLIP: f32 = 0.01;
    /// How bright a probe's reflection may be made.
    pub const INTENSITY: RangeInclusive<f32> = 0.0..=8.0;

    /// A probe at `position` whose box is centred on it with the given half-extents,
    /// with Unity's default settings (near clip 0.05 and far 1000, so a bake captures
    /// every static object).
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

    /// Hold every setting to its range: a power-of-two resolution, a non-negative
    /// blend distance and intensity, and a far clip past the near one.
    pub fn clamp_settings(&mut self) {
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

/// The nearest face size a probe may bake at: a power of two in
/// [`ReflectionProbe::RESOLUTION`] (Unity's resolution menu).
pub fn clamp_resolution(size: u32) -> u32 {
    let (lo, hi) = (
        *ReflectionProbe::RESOLUTION.start(),
        *ReflectionProbe::RESOLUTION.end(),
    );
    let size = size.clamp(lo, hi);
    let up = size.next_power_of_two();
    // Round to the nearer power: 300 → 256, 400 → 512.
    match up - size <= size - up / 2 {
        true => up.min(hi),
        false => up / 2,
    }
}

/// A new probe's sphere radius: Unity's new probe's 10 m box, as a sphere.
pub const DEFAULT_RADIUS: f32 = 5.0;
/// The smallest sphere radius a write accepts.
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

/// A component of a reflection direction at or below this is parallel to that
/// axis's box planes. Shared with the shader's `parallax_correct`.
pub const PARALLEL_EPSILON: f32 = 1e-6;
/// The "no hit" distance of a parallax ray; the shader's `parallax_correct` uses
/// the same value.
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
