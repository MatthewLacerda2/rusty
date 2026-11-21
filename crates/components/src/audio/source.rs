//! Unity's `AudioSource`. Pure authoring data: it knows nothing of the audio device, so the
//! deterministic harness carries it with no audio backend.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioSourceComponent {
    /// Empty means no clip assigned.
    pub clip: String,
    pub volume: f32,
    #[serde(rename = "loop")]
    pub looping: bool,
    pub play_on_start: bool,
    pub is_time_scaled: bool,

    /// 0.0 = fully 2D (stereo kept), 1.0 = fully 3D (stereo downmixed to mono, then panned).
    pub spatial_blend: f32,
    /// Full volume out to this distance, then linear falloff to `final_distance`.
    pub initial_distance: f32,
    /// Silent at and after this distance.
    pub final_distance: f32,

    /// Empty means Master. Read when the voice starts, so a change reroutes the next `Play`.
    #[serde(default)]
    pub output_group: String,

    /// Only a spatialized source (`spatial_blend > 0`) is ever occluded.
    #[serde(default = "occlusion_default")]
    pub occlusion_enabled: bool,

    /// `0` off, `1` physical, above `1` exaggerated; scaled by `spatial_blend`.
    #[serde(default = "doppler_default")]
    pub doppler_level: f32,
}

fn doppler_default() -> f32 {
    1.0
}

fn occlusion_default() -> bool {
    true
}

impl Default for AudioSourceComponent {
    fn default() -> Self {
        Self {
            clip: String::new(),
            volume: 1.0,
            looping: false,
            play_on_start: false,
            is_time_scaled: true,
            spatial_blend: 0.0,
            initial_distance: 1.0,
            final_distance: 16.0,
            output_group: String::new(),
            occlusion_enabled: true,
            doppler_level: 1.0,
        }
    }
}

impl crate::FirstClass for AudioSourceComponent {
    const NAME: &'static str = "AudioSource";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_2d_oneshot_unit_volume() {
        let a = AudioSourceComponent::default();
        assert_eq!(a.volume, 1.0);
        assert!(!a.looping);
        assert!(!a.play_on_start);
        assert!(a.is_time_scaled);
        assert_eq!(a.spatial_blend, 0.0);
        assert!(a.clip.is_empty());
    }

    #[test]
    fn loop_field_serializes_as_loop_keyword() {
        let a = AudioSourceComponent {
            clip: "music/theme.ogg".to_string(),
            looping: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"loop\":true"), "got {json}");
        let back: AudioSourceComponent = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }
}
