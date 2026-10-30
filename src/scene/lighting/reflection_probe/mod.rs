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
//! Storage split (mirrors `skybox_path`): each probe stores a world POSITION, a box
//! VOLUME (`box_min`/`box_max`, the parallax-correction and influence bounds), its
//! settings, and a PATH to its baked cubemap sidecar (a KTX2 file). No GPU data lives
//! in the scene document. `Probe.Bake()` writes those cubemaps (#245); until a probe
//! is baked it may reference a file that doesn't exist yet, and the objects it would
//! cover reflect the skybox.
//!
//! The box-projected parallax correction (the standard Lagarde technique) is a pure
//! function here so it is unit-testable without a GPU; `parallax_correct` in
//! `engine/shaders/lighting/reflection_probe.wgsl` runs the same steps with the same
//! guards (no division by a zero component, no hit at or past [`NO_HIT`]), since WGSL
//! may assume no inf or NaN.
//!
//! This module is pure data + math — deterministic, no RNG/clock.

mod blend;
mod set;

use std::ops::RangeInclusive;

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub use blend::ProbeBlend;
pub use set::ReflectionProbeSet;

/// A single reflection probe: a capture position, the box volume used for parallax
/// correction and blending, Unity's per-probe settings, and the path to its baked
/// cubemap. The cubemap is a referenced sidecar (KTX2) — never inlined into the scene
/// document, mirroring `skybox_path`. A field the document omits takes its default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReflectionProbe {
    /// World-space capture point — the cubemap's origin and the parallax pivot.
    pub position: Vec3,
    /// Min corner of the parallax-correction box (world space).
    pub box_min: Vec3,
    /// Max corner of the parallax-correction box (world space).
    pub box_max: Vec3,
    /// Path to the baked cubemap (a KTX2 file). May be empty / point at a not-yet-baked
    /// file (#245); the runtime then falls back to the global skybox.
    pub cubemap_path: String,
    /// Unity's *Box Projection*: reflections are parallax-corrected against the box,
    /// so they track the room. Off reflects the cube as if infinitely far away.
    pub box_projection: bool,
    /// Unity's *Blend Distance*, world units: how far past its box the probe still
    /// reaches, fading to nothing, so an object between two probes blends them.
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
            cubemap_path: String::new(),
            box_projection: true,
            blend_distance: 1.0,
            importance: 1,
            resolution: None,
            near_clip: 0.05,
            far_clip: 1000.0,
            shadows: true,
            intensity: 1.0,
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
    }

    /// True when `point` lies inside this probe's parallax box (inclusive).
    pub fn contains(&self, point: Vec3) -> bool {
        point.cmpge(self.box_min).all() && point.cmple(self.box_max).all()
    }

    /// Squared distance from `point` to the nearest point of the probe's box, zero
    /// inside it: how the renderer ranks probes for its resident budget (#1238), so a
    /// big room whose capture point is far away still counts as near when the camera
    /// stands in it.
    pub fn box_distance_sq(&self, point: Vec3) -> f32 {
        point
            .clamp(self.box_min, self.box_max)
            .distance_squared(point)
    }

    /// Squared distance from `point` to the probe's capture position — the
    /// blend's tiebreak metric.
    fn distance_sq(&self, point: Vec3) -> f32 {
        self.position.distance_squared(point)
    }

    /// Box-projected parallax correction (Lagarde): given a fragment world position and
    /// a raw reflection direction, intersect the ray against this probe's box and return
    /// the direction from the probe centre to that hit point. This is what makes the
    /// reflection track the room as the camera moves, instead of behaving like an
    /// infinitely-distant skybox. A fragment outside the box, or a ray with no forward
    /// hit, keeps the plain normalized direction. The shader's `parallax_correct` runs
    /// these same steps; with [`Self::box_projection`] off the shader skips it.
    /// Test seam for `tests/reflection_probes.rs`.
    pub fn parallax_correct(&self, world_pos: Vec3, reflection: Vec3) -> Vec3 {
        let dir = reflection.normalize_or_zero();
        if !self.contains(world_pos) {
            return dir;
        }
        // Per-axis ray/box-plane intersection; the forward exit per axis, then the
        // closest of the three. An axis the ray runs parallel to never exits through
        // its planes, so it is set to `NO_HIT` instead of dividing by zero.
        let mut t = NO_HIT;
        for axis in 0..3 {
            if dir[axis].abs() > PARALLEL_EPSILON {
                let first = (self.box_max[axis] - world_pos[axis]) / dir[axis];
                let second = (self.box_min[axis] - world_pos[axis]) / dir[axis];
                t = t.min(first.max(second));
            }
        }
        if t <= 0.0 || t >= NO_HIT {
            return dir;
        }
        let to_hit = world_pos + dir * t - self.position;
        if to_hit.length_squared() <= 0.0 {
            return dir;
        }
        to_hit.normalize()
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
mod tests;
