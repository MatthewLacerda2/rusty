//! src/scene/lighting/reflection_probe.rs — Reflection-probe data model + parallax math.
//!
//! A reflection probe captures the surrounding environment as a prefiltered cubemap
//! so glossy surfaces reflect their actual room instead of the single global skybox.
//! Like light probes (#240), reflection probes are a SCENE-LEVEL DATASET, not a
//! first-class component: an array of probes lives on the scene. Like Unity, the
//! renderer picks one probe per DRAWN OBJECT (#1206): the nearest probe whose box
//! contains the object's world-bounds centre ([`ReflectionProbeSet::select`]), and
//! every fragment of that object reflects that probe's cubemap.
//!
//! This is the SIBLING of the light-probe dataset, kept deliberately DISTINCT:
//!   * light probes (`probe.rs`) carry baked L2 SH irradiance for DIFFUSE ambient;
//!   * reflection probes (here) carry a prefiltered cubemap for SPECULAR reflection.
//!
//! Storage split (mirrors `skybox_path`): each probe stores a world POSITION, a box
//! VOLUME (`box_min`/`box_max`, the parallax-correction bounds), and a PATH to its
//! baked cubemap sidecar (a KTX2 file). No GPU data lives in the scene document — the
//! cubemap is a referenced file, exactly like the skybox. `Probe.Bake()` writes those
//! cubemaps (#245); until a probe is baked it may reference a file that doesn't exist
//! yet, and the objects it would cover reflect the skybox.
//!
//! The probe-selection + box-projected parallax correction (the standard Lagarde
//! technique) is factored into pure functions here so it is unit-testable without a
//! GPU; `parallax_correct` in `engine/shaders/lighting/reflection_probe.wgsl` runs the same
//! steps with the same guards (no division by a zero component, no hit at or past
//! [`NO_HIT`]), since WGSL may assume no inf or NaN.
//!
//! This module is pure data + math — deterministic, no RNG/clock.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// A single reflection probe: a capture position, the box volume used for parallax
/// correction, and the path to its baked cubemap. The cubemap is a referenced sidecar
/// (KTX2) — never inlined into the scene document, mirroring `skybox_path`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReflectionProbe {
    /// World-space capture point — the cubemap's origin and the parallax pivot.
    pub position: Vec3,
    /// Min corner of the parallax-correction box (world space).
    pub box_min: Vec3,
    /// Max corner of the parallax-correction box (world space).
    pub box_max: Vec3,
    /// Path to the baked cubemap (a KTX2 file). May be empty / point at a not-yet-baked
    /// file (#245); the runtime then falls back to the global skybox.
    #[serde(default)]
    pub cubemap_path: String,
}

impl ReflectionProbe {
    /// A probe at `position` whose box is centred on it with the given half-extents.
    pub fn new(position: Vec3, half_extents: Vec3) -> Self {
        let he = half_extents.abs();
        Self {
            position,
            box_min: position - he,
            box_max: position + he,
            cubemap_path: String::new(),
        }
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
    /// nearest-probe tiebreak metric.
    fn distance_sq(&self, point: Vec3) -> f32 {
        self.position.distance_squared(point)
    }

    /// Box-projected parallax correction (Lagarde): given a fragment world position and
    /// a raw reflection direction, intersect the ray against this probe's box and return
    /// the direction from the probe centre to that hit point. This is what makes the
    /// reflection track the room as the camera moves, instead of behaving like an
    /// infinitely-distant skybox. A fragment outside the box, or a ray with no forward
    /// hit, keeps the plain normalized direction. The shader's `parallax_correct` runs
    /// these same steps.
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

/// A component of a reflection direction at or below this is parallel to that
/// axis's box planes. Shared with the shader's `parallax_correct`.
pub const PARALLEL_EPSILON: f32 = 1e-6;
/// The "no hit" distance of a parallax ray; the shader's `parallax_correct` uses
/// the same value.
pub const NO_HIT: f32 = 1e30;

/// The scene-level reflection-probe dataset. Owned by `Scene`, serialized into the
/// scene document (positions + boxes + cubemap paths; no GPU data).
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

    /// Pick the reflection probe that applies at `world_pos`, with its index: the
    /// NEAREST probe (by capture distance) whose box contains the point, the lower
    /// index in this set on a tie (the renderer keeps its resident set in scene order,
    /// so that is the lower scene index there too, wherever the camera is, #1238).
    /// `None` when no probe's box covers the point — the caller then falls back to
    /// the global skybox. The renderer calls it per draw, at the
    /// object's world-bounds centre (#1206).
    pub fn select(&self, world_pos: Vec3) -> Option<(usize, &ReflectionProbe)> {
        self.probes
            .iter()
            .enumerate()
            .filter(|(_, p)| p.contains(world_pos))
            .min_by(|(_, a), (_, b)| {
                a.distance_sq(world_pos)
                    .total_cmp(&b.distance_sq(world_pos))
            })
    }
}

#[cfg(test)]
#[path = "reflection_probe_tests.rs"]
mod reflection_probe_tests;
