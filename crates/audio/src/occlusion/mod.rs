mod maestro;

use std::collections::BTreeMap;

use glam::Vec3;

use super::backend::VoiceId;
use super::mixer::settings::LOW_PASS_OFF;

/// Whether the segment `from → to` is blocked, skipping entity `ignore` (and its
/// ancestors) and any collider whose layer is not in the `mask`.
pub type Occluder = Box<dyn Fn(Vec3, Vec3, Option<u32>, u32) -> bool>;

/// The voice's gain at full occlusion (about −9 dB).
pub const OCCLUDED_GAIN: f32 = 0.35;
/// The voice's low-pass cutoff at full occlusion, in Hz.
pub const OCCLUDED_CUTOFF: f32 = 800.0;
/// Seconds (unscaled) for the factor to travel the whole `0 ↔ 1` range.
pub const FADE_SECONDS: f32 = 0.25;
pub const SOURCE_SPREAD: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionSettings {
    pub strength: f32,
    pub layer_mask: u32,
    pub voices_per_tick: u32,
}

impl Default for OcclusionSettings {
    fn default() -> Self {
        Self {
            strength: 1.0,
            layer_mask: u32::MAX,
            voices_per_tick: 16,
        }
    }
}

pub fn effect(occlusion: f32) -> (f32, f32) {
    let o = occlusion.clamp(0.0, 1.0);
    if o <= 0.0 {
        return (1.0, LOW_PASS_OFF);
    }
    let gain = 1.0 - o * (1.0 - OCCLUDED_GAIN);
    (
        gain,
        LOW_PASS_OFF * (OCCLUDED_CUTOFF / LOW_PASS_OFF).powf(o),
    )
}

pub fn ray_targets(listener: Vec3, source: Vec3) -> [Vec3; 3] {
    let side = (source - listener)
        .cross(Vec3::Y)
        .try_normalize()
        .unwrap_or(Vec3::X)
        * SOURCE_SPREAD;
    [source, source - side, source + side]
}

pub fn cast(listener: Vec3, source: Vec3, blocked: impl Fn(Vec3, Vec3) -> bool) -> f32 {
    let targets = ray_targets(listener, source);
    let hits = targets.iter().filter(|&&t| blocked(listener, t)).count();
    hits as f32 / targets.len() as f32
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VoiceOcclusion {
    pub target: f32,
    pub value: f32,
}

impl VoiceOcclusion {
    pub fn at(value: f32) -> Self {
        Self {
            target: value,
            value,
        }
    }

    /// Move the factor toward the target over `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let reach = dt.max(0.0) / FADE_SECONDS;
        let delta = (self.target - self.value).clamp(-reach, reach);
        self.value += delta;
    }
}

#[derive(Default)]
pub struct Occlusion {
    pub(super) settings: OcclusionSettings,
    pub(super) occluder: Option<Occluder>,
    pub(super) listener: Vec3,
    pub(super) voices: BTreeMap<VoiceId, VoiceOcclusion>,
    pub(super) cursor: Option<VoiceId>,
}

impl Occlusion {
    pub fn factor(&self, id: VoiceId) -> f32 {
        self.voices.get(&id).map_or(0.0, |v| v.value)
    }

    pub fn applied(&self, id: VoiceId) -> f32 {
        self.factor(id) * self.settings.strength
    }
}

#[cfg(test)]
#[path = "occlusion_tests.rs"]
mod occlusion_tests;
