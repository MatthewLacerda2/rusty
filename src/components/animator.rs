//! src/components/animator.rs — Animator component
//!
//! The runtime state of an entity's skeletal animation: which clip is playing, the
//! playhead, speed, and a minimal current-state machine for crossfades (#80). The
//! clips themselves live on the entity's skinned `MeshComponent` (rehydrated from
//! the imported source, never serialized); this component only references one by
//! name and tracks where the playhead is. Unity analog: `Animator`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorComponent {
    /// Name of the clip currently playing (matched against the mesh's clip list).
    pub current_clip: String,
    /// Playhead, in clip-local seconds. Advanced by the `animate` system each frame
    /// while playing; the keyframe sampler poses the skeleton at this time.
    pub time: f32,
    /// Playback rate multiplier applied to the fixed timestep.
    pub speed: f32,
    pub is_playing: bool,
    /// Editor/debug hold: freezes the playhead without clearing `is_playing`.
    pub freeze: bool,
    /// The clip being faded *out* during a crossfade, with its own playhead. `None`
    /// outside a crossfade. Sampling blends this pose into `current_clip`'s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_clip: Option<String>,
    /// Playhead of `previous_clip` during a crossfade, frozen at the moment the
    /// crossfade began (Unity samples the outgoing clip from where it was).
    #[serde(default)]
    pub previous_time: f32,
    /// Seconds elapsed in the active crossfade. Reaches `crossfade_duration`, then
    /// the crossfade ends and `previous_clip` is cleared.
    #[serde(default)]
    pub crossfade_elapsed: f32,
    /// Total crossfade length in seconds; `0.0` means no crossfade is active.
    #[serde(default)]
    pub crossfade_duration: f32,
}

impl Default for AnimatorComponent {
    fn default() -> Self {
        Self {
            current_clip: String::new(),
            time: 0.0,
            speed: 1.0,
            is_playing: false,
            freeze: false,
            previous_clip: None,
            previous_time: 0.0,
            crossfade_elapsed: 0.0,
            crossfade_duration: 0.0,
        }
    }
}

impl AnimatorComponent {
    /// Start `clip` immediately, discarding any crossfade — a hard cut from the
    /// current pose. Resets the playhead so the clip plays from its start.
    pub fn play(&mut self, clip: String) {
        self.current_clip = clip;
        self.time = 0.0;
        self.is_playing = true;
        self.freeze = false;
        self.previous_clip = None;
        self.crossfade_elapsed = 0.0;
        self.crossfade_duration = 0.0;
    }

    /// Crossfade into `clip` over `duration` seconds, blending out the clip that is
    /// playing now. A non-positive `duration`, or a fade into the already-current
    /// clip, degrades to a plain [`play`](Self::play).
    pub fn crossfade(&mut self, clip: String, duration: f32) {
        if duration <= 0.0 || clip == self.current_clip {
            self.play(clip);
            return;
        }
        self.previous_clip = Some(std::mem::replace(&mut self.current_clip, clip));
        self.previous_time = self.time;
        self.time = 0.0;
        self.crossfade_elapsed = 0.0;
        self.crossfade_duration = duration;
        self.is_playing = true;
        self.freeze = false;
    }

    /// The crossfade weight in `[0, 1]`: 0 fully on `previous_clip`, 1 fully on
    /// `current_clip`. `1.0` when no crossfade is active.
    pub fn crossfade_weight(&self) -> f32 {
        if self.crossfade_duration <= 0.0 {
            return 1.0;
        }
        (self.crossfade_elapsed / self.crossfade_duration).clamp(0.0, 1.0)
    }

    /// True while a crossfade is blending (a previous clip is still contributing).
    pub fn is_crossfading(&self) -> bool {
        self.previous_clip.is_some() && self.crossfade_duration > 0.0
    }

    /// Advance both playheads (and any crossfade) by `dt` scaled seconds. Ending the
    /// crossfade clears the previous clip so sampling falls back to a single pose.
    pub fn advance(&mut self, dt: f32) {
        if !self.is_playing || self.freeze {
            return;
        }
        let step = dt * self.speed;
        self.time += step;
        if self.is_crossfading() {
            self.previous_time += step;
            self.crossfade_elapsed += dt;
            if self.crossfade_elapsed >= self.crossfade_duration {
                self.previous_clip = None;
                self.crossfade_duration = 0.0;
                self.crossfade_elapsed = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests;
