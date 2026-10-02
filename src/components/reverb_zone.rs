//! src/components/reverb_zone.rs — AudioReverbZone component (#469).
//!
//! Unity's `AudioReverbZone`: a sphere around the entity that sets the world mix's
//! reverb while the listener is inside it. Full effect within `min_distance`, fading
//! linearly to nothing at `max_distance`; where zones overlap their parameters blend
//! by weight (`audio::reverb`). A zone carries a preset name plus the raw
//! [`ReverbParams`] that are always the authority: picking a preset writes its
//! params, editing a param switches the preset to `Custom` (Unity's `User`).
//!
//! Pure authoring data — every field serde-persists. The sim resolves the
//! listener's blend each `LateUpdate`; the device renders it on the reverb bus.

use serde::{Deserialize, Serialize};

/// Longest decay a zone may ask for, in seconds (Unity's `decayTime` range).
pub const DECAY_RANGE: (f32, f32) = (0.1, 20.0);
/// Longest pre-delay, in seconds (Unity's `reflectionsDelay` cap).
pub const MAX_PRE_DELAY: f32 = 0.3;

/// The reverb a zone asks for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReverbParams {
    /// How long the tail rings, in seconds (time to fall 60 dB).
    pub decay_time: f32,
    /// Gap between the dry sound and the tail's onset, in seconds.
    pub pre_delay: f32,
    /// How fast high frequencies die in the tail, `0..1` (`0` is bright).
    pub damping: f32,
    /// How loud the tail is, linear `0..1` (`0` is dry).
    pub wet: f32,
}

impl ReverbParams {
    /// No reverb: what the listener hears outside every zone.
    pub const DRY: ReverbParams = ReverbParams {
        decay_time: 1.0,
        pre_delay: 0.0,
        damping: 0.5,
        wet: 0.0,
    };

    /// These params clamped into range; a NaN reads as the range's floor.
    pub fn clamped(self) -> Self {
        let fix = |v: f32, lo: f32, hi: f32| if v.is_nan() { lo } else { v.clamp(lo, hi) };
        Self {
            decay_time: fix(self.decay_time, DECAY_RANGE.0, DECAY_RANGE.1),
            pre_delay: fix(self.pre_delay, 0.0, MAX_PRE_DELAY),
            damping: fix(self.damping, 0.0, 1.0),
            wet: fix(self.wet, 0.0, 1.0),
        }
    }
}

/// A named starting point for a zone's params (Unity's `AudioReverbPreset`, cut
/// down to the rooms a shooter needs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReverbPreset {
    /// Dry: a zone that cancels the reverb around it.
    Off,
    /// A small room: short and dense.
    #[default]
    Room,
    /// A large hall: long and smooth.
    Hall,
    /// A concrete tunnel: long, bright tail.
    Tunnel,
    /// Open air: almost dry, a faint late echo.
    Outdoor,
    /// Hand-set params (Unity's `User`).
    Custom,
}

impl ReverbPreset {
    /// Every preset, in menu order.
    pub const ALL: [ReverbPreset; 6] = [
        Self::Off,
        Self::Room,
        Self::Hall,
        Self::Tunnel,
        Self::Outdoor,
        Self::Custom,
    ];

    /// The preset's name, as the API and inspector show it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Room => "Room",
            Self::Hall => "Hall",
            Self::Tunnel => "Tunnel",
            Self::Outdoor => "Outdoor",
            Self::Custom => "Custom",
        }
    }

    /// The preset called `name` (case-insensitive; `User` is Unity's `Custom`).
    pub fn parse(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower == "user" {
            return Some(Self::Custom);
        }
        Self::ALL
            .into_iter()
            .find(|p| p.name().to_lowercase() == lower)
    }

    /// The params this preset stands for; `None` for `Custom`, which has none.
    pub fn params(self) -> Option<ReverbParams> {
        let p = |decay_time, pre_delay, damping, wet| ReverbParams {
            decay_time,
            pre_delay,
            damping,
            wet,
        };
        match self {
            Self::Off => Some(ReverbParams::DRY),
            Self::Room => Some(p(0.4, 0.005, 0.5, 0.35)),
            Self::Hall => Some(p(1.8, 0.02, 0.3, 0.45)),
            Self::Tunnel => Some(p(2.8, 0.015, 0.1, 0.55)),
            Self::Outdoor => Some(p(1.0, 0.04, 0.7, 0.08)),
            Self::Custom => None,
        }
    }
}

/// A reverb zone. Mirrors Unity's `AudioReverbZone` (sphere, min/max distance).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReverbZoneComponent {
    /// Radius of full effect, in metres.
    pub min_distance: f32,
    /// Radius where the effect has faded to nothing; never under `min_distance`.
    pub max_distance: f32,
    /// The preset the params came from (`Custom` once one is hand-edited).
    pub preset: ReverbPreset,
    /// The reverb this zone applies; the authority over `preset`.
    pub params: ReverbParams,
}

impl Default for ReverbZoneComponent {
    /// Unity's fresh zone (10 m full, 15 m faded) on the `Room` preset.
    fn default() -> Self {
        let preset = ReverbPreset::Room;
        Self {
            min_distance: 10.0,
            max_distance: 15.0,
            preset,
            params: preset.params().unwrap_or(ReverbParams::DRY),
        }
    }
}
