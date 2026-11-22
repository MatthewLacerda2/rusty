use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::PROJECT_FILE;

/// The default speed of sound, in m/s (dry air at 20 °C, Unity's and FMOD's).
pub const SPEED_OF_SOUND: f32 = 343.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AudioSettings {
    pub speed_of_sound: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            speed_of_sound: SPEED_OF_SOUND,
        }
    }
}

impl AudioSettings {
    pub fn read_json(blob: &Value) -> (Self, Vec<String>) {
        let mut settings = Self::default();
        let Some(map) = blob.as_object() else {
            return (settings, vec![format!("expected an object, found {blob}")]);
        };
        let mut ignored = Vec::new();
        for (key, value) in map {
            if key != "speed_of_sound" {
                ignored.push(format!("unknown key '{key}' (one of speed_of_sound)"));
                continue;
            }
            match value.as_f64().map(|c| c as f32) {
                Some(c) if c.is_finite() && c > 0.0 => settings.speed_of_sound = c,
                _ => ignored.push(format!(
                    "speed_of_sound is {value}, not a positive number; using {SPEED_OF_SOUND}"
                )),
            }
        }
        (settings, ignored)
    }
}

impl<'de> Deserialize<'de> for AudioSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (settings, ignored) = Self::read_json(&Value::deserialize(deserializer)?);
        for line in ignored {
            log::warn!("[Project] {PROJECT_FILE} audio: {line}");
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectFile;

    #[test]
    fn a_partial_or_bad_block_keeps_the_default() {
        let read = |json: &str| AudioSettings::read_json(&serde_json::from_str(json).unwrap());
        assert_eq!(read("{}"), (AudioSettings::default(), vec![]));
        assert_eq!(read(r#"{"speed_of_sound": 100}"#).0.speed_of_sound, 100.0);
        for bad in ["-5", "0", r#""343""#, "null", "true"] {
            let (settings, ignored) = read(&format!(r#"{{"speed_of_sound": {bad}}}"#));
            assert_eq!(settings.speed_of_sound, SPEED_OF_SOUND, "{bad}");
            assert_eq!(ignored.len(), 1, "{bad} is warned about");
        }
        assert_eq!(read(r#"{"sped": 1}"#).1.len(), 1, "a typo is warned about");
    }

    #[test]
    fn a_bad_value_still_opens_the_project_file() {
        for bad in [r#""343""#, "null", "-1"] {
            let json = format!(r#"{{"audio": {{"speed_of_sound": {bad}}}}}"#);
            let file = ProjectFile::from_json(&json).expect(bad);
            assert_eq!(file.audio.speed_of_sound, SPEED_OF_SOUND, "{bad}");
        }
    }
}
