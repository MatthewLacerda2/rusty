//! src/audio/occlusion/ — muffle and attenuate voices behind geometry (#467).
//!
//! For each spatialized voice (`spatial_blend > 0`, `occlusion_enabled`), rays are
//! cast through the physics world from the listener to the source. The fraction
//! that is blocked is the voice's **occlusion factor** in `[0, 1]`; it lowers the
//! voice's gain and closes a per-voice low-pass (`device/lowpass.rs`), so a shot
//! behind a wall sounds muffled.
//!
//! - **Rays: three, a horizontal fan** (Steam Audio's volumetric occlusion, cut to
//!   what a corridor shooter needs): one to the source, one [`SOURCE_SPREAD`] to
//!   each side of it across the line of sight. Stepping past a door frame takes
//!   the factor through ⅓ and ⅔ rather than flipping it. The fan stays level so
//!   a footstep on the floor never has a ray go into the floor under it.
//! - **Cadence.** At most `voices_per_tick` voices are re-cast each `LateUpdate`,
//!   round-robin in voice order, so the cost stays bounded with many voices. A new
//!   voice is cast the moment it starts, before its first sample plays: a shot
//!   behind a wall is muffled from its first transient.
//! - **Smoothing.** The factor moves toward the latest cast's value linearly over
//!   [`FADE_SECONDS`] of unscaled time, so a source never pops as the player
//!   steps around a corner. A new voice starts at its cast value.
//!
//! The physics query is not done here: the app hands the maestro an [`Occluder`],
//! so the audio module stays free of the physics world and the harness runs the
//! same casts as the windowed game. Everything is sim-side and deterministic, and
//! `Audio.GetSpatial` reads the factor back.
//!
//! Out of scope: diffraction, portals, material transmission (one fixed muffle).

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
/// How far either side of the source the outer rays aim, in metres.
pub const SOURCE_SPREAD: f32 = 0.5;

/// The global occlusion settings (`Audio.SetOcclusionSettings`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionSettings {
    /// How much of the effect applies, `0..1`; `0` turns occlusion off (no casts).
    pub strength: f32,
    /// Which layers occlude (bit per layer); clear the bits of characters and
    /// other things sound should pass.
    pub layer_mask: u32,
    /// How many voices are re-cast per `LateUpdate` (at least 1).
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

/// What an occlusion factor does to a voice: `(gain multiplier, low-pass cutoff)`.
/// `0` is untouched (cutoff [`LOW_PASS_OFF`], the filter bypassed); `1` is
/// [`OCCLUDED_GAIN`] and [`OCCLUDED_CUTOFF`], the cutoff moving in octaves.
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

/// The three ray targets: the source, then either side of it across the line of
/// sight, level with it.
pub fn ray_targets(listener: Vec3, source: Vec3) -> [Vec3; 3] {
    let side = (source - listener)
        .cross(Vec3::Y)
        .try_normalize()
        .unwrap_or(Vec3::X)
        * SOURCE_SPREAD;
    [source, source - side, source + side]
}

/// The fraction of the three rays from `listener` that `blocked` reports blocked.
pub fn cast(listener: Vec3, source: Vec3, blocked: impl Fn(Vec3, Vec3) -> bool) -> f32 {
    let targets = ray_targets(listener, source);
    let hits = targets.iter().filter(|&&t| blocked(listener, t)).count();
    hits as f32 / targets.len() as f32
}

/// One voice's occlusion: the latest cast and the smoothed factor heading to it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VoiceOcclusion {
    pub target: f32,
    pub value: f32,
}

impl VoiceOcclusion {
    /// A voice starting at its first cast.
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

/// The maestro's occlusion state: settings, the occluder, and every voice's factor.
#[derive(Default)]
pub struct Occlusion {
    pub(super) settings: OcclusionSettings,
    pub(super) occluder: Option<Occluder>,
    /// The listener position of the last `LateUpdate`, which play-time casts use.
    pub(super) listener: Vec3,
    pub(super) voices: BTreeMap<VoiceId, VoiceOcclusion>,
    /// The voice the last round-robin pass ended on.
    pub(super) cursor: Option<VoiceId>,
}

impl Occlusion {
    /// Voice `id`'s smoothed factor (`0` for an untracked voice).
    pub fn factor(&self, id: VoiceId) -> f32 {
        self.voices.get(&id).map_or(0.0, |v| v.value)
    }

    /// Voice `id`'s factor scaled by the global strength — what the mix applies.
    pub fn applied(&self, id: VoiceId) -> f32 {
        self.factor(id) * self.settings.strength
    }
}

#[cfg(test)]
#[path = "occlusion_tests.rs"]
mod occlusion_tests;
