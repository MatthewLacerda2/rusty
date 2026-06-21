//! src/components/audio_source.rs — AudioSource component (2D playback, #212).
//!
//! Unity's `AudioSource`: a per-entity emitter referencing an audio `clip` to play.
//! This issue (#212) ships **2D (non-spatialized) playback**; the spatial fields
//! (`spatial_blend`, `initial_distance`, `final_distance`) are *stored now* and
//! consumed by the 3D-spatialization follow-up (#213), so a scene authored today
//! round-trips unchanged when that lands.
//!
//! Pure authoring data — every field serde-persists. The component knows nothing
//! about the audio device: the platform-layer `AudioMaestro` (the resource) owns
//! the mixer and turns these fields into actual sound. Keeping the component
//! device-free is what lets the deterministic harness carry `AudioSource`s with no
//! audio backend at all.

use serde::{Deserialize, Serialize};

/// Per-entity audio emitter. Mirrors Unity's `AudioSource`. All fields persist; the
/// `AudioMaestro` reads them when (re)starting a voice for this entity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioSourceComponent {
    /// Asset path to the clip (`.ogg` / `.wav`). Empty means "no clip assigned".
    pub clip: String,
    /// Per-source linear gain in `[0, 1]`, multiplied by the master volume.
    pub volume: f32,
    /// Whether playback loops when it reaches the end.
    #[serde(rename = "loop")]
    pub looping: bool,
    /// Start playing automatically when the entity enters Play (Unity's "Play On
    /// Awake").
    pub play_on_start: bool,
    /// `true` ⇒ the voice's rate follows `Time.timeScale` (slows in slow-mo, stops
    /// when paused) — for diegetic gameplay sound. `false` ⇒ wall-clock rate, for
    /// music and UI that should keep playing through a pause/slow-mo.
    pub is_time_scaled: bool,

    // --- Spatial fields: STORED NOW, consumed by #213 (3D spatialization). ---
    /// 0.0 = fully 2D (no attenuation/panning), 1.0 = fully 3D. Stored only here.
    pub spatial_blend: f32,
    /// Rolloff: full volume out to this distance (model: flat ≤ initial, linear
    /// falloff initial→final, silent beyond). Stored now; applied by #213.
    pub initial_distance: f32,
    /// Rolloff: silent at/after this distance. Stored now; applied by #213.
    pub final_distance: f32,
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
        }
    }
}

impl AudioSourceComponent {
    /// Whether a clip path is set (an empty path is "no clip").
    pub fn has_clip(&self) -> bool {
        !self.clip.trim().is_empty()
    }
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
        assert!(!a.has_clip());
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
