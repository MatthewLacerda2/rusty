use serde_json::Value;

use super::backend::VoiceMix;
use rusty_core::storage::Storage;

pub const AUDIO_NAMESPACE: &str = "audio";
pub const SPEAKER_MODE_KEY: &str = "speaker_mode";

/// How much of a voice's pan survives in headphones mode: a hard-left voice pans to
/// `-0.6`, so the right ear still hears it at 40 %.
pub const HEADPHONE_WIDTH: f32 = 0.6;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum SpeakerMode {
        Headphones = "headphones",
        Tv = "tv",
        #[default]
        HomeTheater = "home_theater",
    }
}

impl SpeakerMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Headphones => "Headphones",
            Self::Tv => "TV",
            Self::HomeTheater => "Home theater",
        }
    }

    pub fn shape(self, mix: VoiceMix) -> VoiceMix {
        match self {
            Self::Headphones => VoiceMix {
                pan: mix.pan * HEADPHONE_WIDTH,
                ..mix
            },
            Self::Tv | Self::HomeTheater => mix,
        }
    }

    pub fn load(storage: &Storage) -> Self {
        storage
            .get(AUDIO_NAMESPACE, SPEAKER_MODE_KEY)
            .and_then(|v| v.as_str().and_then(Self::parse))
            .unwrap_or_default()
    }

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
