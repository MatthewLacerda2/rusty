//! Unity's `AudioReverbZone`: full effect within `min_distance`, linear fade to `max_distance`.
//! The raw [`ReverbParams`] are the authority; editing a param switches the preset to `Custom`.

use serde::{Deserialize, Serialize};

// In seconds: Unity's `decayTime` range and `reflectionsDelay` cap.
pub const DECAY_RANGE: (f32, f32) = (0.1, 20.0);
pub const MAX_PRE_DELAY: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReverbParams {
    /// Seconds to fall 60 dB.
    pub decay_time: f32,
    /// Seconds.
    pub pre_delay: f32,
    /// `0..1`, `0` is bright.
    pub damping: f32,
    /// Linear `0..1`, `0` is dry.
    pub wet: f32,
}

impl ReverbParams {
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

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ReverbPreset {
        Off = "Off",
        #[default]
        Room = "Room",
        Hall = "Hall",
        Tunnel = "Tunnel",
        Outdoor = "Outdoor",
        Custom = "Custom" | "User",
    }
}

impl ReverbPreset {
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReverbZoneComponent {
    pub min_distance: f32,
    /// Radius where the effect has faded to nothing; never under `min_distance`.
    pub max_distance: f32,
    pub preset: ReverbPreset,
    pub params: ReverbParams,
}

impl Default for ReverbZoneComponent {
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

impl crate::FirstClass for ReverbZoneComponent {
    const NAME: &'static str = "AudioReverbZone";
}
