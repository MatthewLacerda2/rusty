//! src/audio/speaker.rs — the speaker-mode setting: output profiles (#546).
//!
//! One global, player-facing setting that fits the final mix to the listening setup,
//! the "speaker mode" / "dynamic range" option shipped games carry:
//!
//!   * **Home theater** (the default): the reference mix, untouched. A game that
//!     never touches the setting sounds exactly as authored.
//!   * **Headphones**: a narrower stereo image. Each voice's pan is scaled by
//!     [`HEADPHONE_WIDTH`], so a hard-panned 3D source never sits in one ear only,
//!     and the master bus adds a light crossfeed for clips that are wide on their own
//!     (music, stereo ambience).
//!   * **TV**: dynamic range compression on the master bus. Quiet cues (footsteps,
//!     reloads) come up and loud peaks are held down, so they stay audible over room
//!     noise on small speakers.
//!
//! The per-voice half ([`SpeakerMode::shape`]) is here and applied by the maestro;
//! the summed-output half is the device's master bus (`device/master.rs`). Both are
//! output shaping in the platform layer: `FixedUpdate` never reads the mode, so the
//! sim stays deterministic. Stereo only; true surround is out of scope.
//!
//! Persisted through [`Storage`] as `audio.speaker_mode` (a mode name), loaded at
//! startup and written back at Stop / quit by `shell/settings.rs`.

use serde_json::Value;

use super::backend::VoiceMix;
use crate::core::storage::Storage;

/// The `Storage` namespace audio settings persist under.
pub const AUDIO_NAMESPACE: &str = "audio";
/// The key the speaker mode persists under, inside [`AUDIO_NAMESPACE`].
pub const SPEAKER_MODE_KEY: &str = "speaker_mode";

/// How much of a voice's pan survives in headphones mode: a hard-left voice pans to
/// `-0.6`, so the right ear still hears it at 40 %.
pub const HEADPHONE_WIDTH: f32 = 0.6;

/// The output profile the final mix is shaped for. See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpeakerMode {
    Headphones,
    Tv,
    #[default]
    HomeTheater,
}

impl SpeakerMode {
    /// Every mode, in menu order.
    pub const ALL: [SpeakerMode; 3] = [Self::Headphones, Self::Tv, Self::HomeTheater];

    /// The script / storage name: `headphones`, `tv`, `home_theater`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Headphones => "headphones",
            Self::Tv => "tv",
            Self::HomeTheater => "home_theater",
        }
    }

    /// The editor label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Headphones => "Headphones",
            Self::Tv => "TV",
            Self::HomeTheater => "Home theater",
        }
    }

    /// The mode named `name` (case-insensitive), or `None`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }

    /// The mode's per-voice shaping of an output mix. Headphones narrows the pan;
    /// the other modes hand the mix back unchanged (TV works on the summed output).
    pub fn shape(self, mix: VoiceMix) -> VoiceMix {
        match self {
            Self::Headphones => VoiceMix {
                pan: mix.pan * HEADPHONE_WIDTH,
                ..mix
            },
            Self::Tv | Self::HomeTheater => mix,
        }
    }

    /// The persisted mode; a missing or unrecognised value is the default.
    pub fn load(storage: &Storage) -> Self {
        storage
            .get(AUDIO_NAMESPACE, SPEAKER_MODE_KEY)
            .and_then(|v| v.as_str().and_then(Self::parse))
            .unwrap_or_default()
    }

    /// Write this mode into `storage` (the Stop / quit flush puts it on disk).
    pub fn store(self, storage: &mut Storage) {
        storage.set(
            AUDIO_NAMESPACE,
            SPEAKER_MODE_KEY,
            Value::String(self.name().to_string()),
        );
    }
}

#[cfg(test)]
#[path = "speaker_tests.rs"]
mod speaker_tests;
