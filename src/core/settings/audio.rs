//! src/core/settings/audio.rs — `AudioSettings`: the project's audio settings (#1749).
//!
//! Unity's *Project Settings → Audio*, the part rusty has: the speed of sound the
//! doppler shift resolves with (`audio::doppler`). The values live in the tracked
//! `project.rusty` under `"audio"`, are applied to the `AudioMaestro` on entering
//! Play and on Stop, and are edited in the Build Settings window;
//! `Audio.SetSpeedOfSound` changes the running game's until Stop.

use serde::{Deserialize, Serialize};

/// The default speed of sound, in m/s (dry air at 20 °C, Unity's and FMOD's).
pub const SPEED_OF_SOUND: f32 = 343.0;

/// The project's audio settings. A partial `"audio"` block keeps the defaults for
/// what it omits.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// The speed of sound in m/s, which the doppler shift divides by (343, dry air).
    pub speed_of_sound: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            speed_of_sound: SPEED_OF_SOUND,
        }
    }
}
