//! src/core/settings/audio.rs — `AudioSettings`: the project's audio settings (#1749).
//!
//! Unity's *Project Settings → Audio*, the part rusty has: the speed of sound the
//! doppler shift resolves with (`audio::doppler`). The values live in the tracked
//! `project.rusty` under `"audio"`, are applied to the `AudioMaestro` on entering
//! Play and on Stop, and are edited in the Build Settings window;
//! `Audio.SetSpeedOfSound` changes the running game's until Stop.

use serde::{Deserialize, Deserializer, Serialize};

use crate::core::project::PROJECT_FILE;

/// The default speed of sound, in m/s (dry air at 20 °C, Unity's and FMOD's).
pub const SPEED_OF_SOUND: f32 = 343.0;

/// The project's audio settings. A partial `"audio"` block keeps the defaults for
/// what it omits.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// The speed of sound in m/s, which the doppler shift divides by (343, dry air).
    /// A value that is not positive keeps the default, with a warning.
    #[serde(deserialize_with = "positive")]
    pub speed_of_sound: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            speed_of_sound: SPEED_OF_SOUND,
        }
    }
}

/// A positive speed of sound, else the default and a warning naming the value.
fn positive<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let c = f32::deserialize(deserializer)?;
    if c.is_finite() && c > 0.0 {
        return Ok(c);
    }
    log::warn!("[Project] {PROJECT_FILE} audio: speed_of_sound {c} is not positive; using {SPEED_OF_SOUND}");
    Ok(SPEED_OF_SOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_or_bad_block_keeps_the_default() {
        let read = |json: &str| serde_json::from_str::<AudioSettings>(json).unwrap();
        assert_eq!(read("{}").speed_of_sound, SPEED_OF_SOUND);
        assert_eq!(read(r#"{"speed_of_sound": 100}"#).speed_of_sound, 100.0);
        assert_eq!(
            read(r#"{"speed_of_sound": -5}"#).speed_of_sound,
            SPEED_OF_SOUND
        );
        assert_eq!(
            read(r#"{"speed_of_sound": 0}"#).speed_of_sound,
            SPEED_OF_SOUND
        );
    }
}
