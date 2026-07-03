//! src/components/animator.rs — Animator component
//!
//! The runtime state of an entity's skeletal animation: which clip is playing, the
//! playhead, speed, and a minimal current-state machine for crossfades (#80). The
//! clips themselves live on the entity's skinned `MeshComponent` (rehydrated from
//! the imported source, never serialized); this component only references one by
//! name and tracks where the playhead is. Unity analog: `Animator`.

use serde::{Deserialize, Serialize};

mod graph_state;
mod parameters;
pub use parameters::{AnimatorParameter, AnimatorParameters};

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
    /// Pause hold (`Animator.Pause`/`Resume`, the editor's Freeze toggle): freezes
    /// the playhead without clearing `is_playing`, so the pose holds and playback
    /// resumes from the same frame.
    pub freeze: bool,
    /// Loop the current clip: [`advance`](Self::advance) wraps the playhead modulo
    /// the clip's duration so it repeats seamlessly. Off, the playhead runs past the
    /// end and the sampler holds the last frame. Persists across
    /// [`play`](Self::play)/[`crossfade`](Self::crossfade), like `speed` (#313).
    #[serde(default)]
    pub loop_clip: bool,
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
    /// Typed graph parameters (#314): the named `Bool`/`Float`/`Int`/`Trigger`
    /// values scripts set (`Animator.Set*`) and the `AnimationGraph`'s edge
    /// conditions will read (#315/#316). Values live here and serialize with the
    /// entity; declarations belong to the graph asset. Name-sorted (`BTreeMap`) so
    /// the evaluator's fixed-step walk is deterministic. The typed accessors
    /// (`set_bool` … `consume_trigger`) live in the `parameters` submodule.
    #[serde(default, skip_serializing_if = "AnimatorParameters::is_empty")]
    pub parameters: AnimatorParameters,
    /// Path to the `AnimationGraph` asset (`guard.animgraph`, #315) driving this
    /// animator — a path-based reference, exactly like a mesh references its source
    /// file, so the scene stores the reference and the graph is rehydrated on load.
    /// `None` means no graph: the animator only does what scripts tell it. The
    /// evaluator that walks the graph each fixed step is #316.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
    /// Auto-evaluate the referenced graph each fixed step (#316). On by default;
    /// off (`Animator.SetGraphEnabled(id, false)`), the graph is inert data and
    /// the animator stays under direct `Play`/`Crossfade` control.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub graph_enabled: bool,
    /// Name of the graph node (state) currently active (#316). `None` until the
    /// evaluator binds the graph — its first step seeds the declared parameter
    /// defaults and enters the entry node — or after the graph reference changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_node: Option<String>,
    /// The active graph node's per-node playback-rate multiplier, stacked on the
    /// component's own [`speed`](Self::speed) by [`advance`](Self::advance).
    /// `1.0` outside a graph or for a node with no speed override.
    #[serde(
        default = "default_node_speed",
        skip_serializing_if = "is_default_speed"
    )]
    pub node_speed: f32,
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn default_node_speed() -> f32 {
    1.0
}

fn is_default_speed(v: &f32) -> bool {
    *v == 1.0
}

impl Default for AnimatorComponent {
    fn default() -> Self {
        Self {
            current_clip: String::new(),
            time: 0.0,
            speed: 1.0,
            is_playing: false,
            freeze: false,
            loop_clip: false,
            previous_clip: None,
            previous_time: 0.0,
            crossfade_elapsed: 0.0,
            crossfade_duration: 0.0,
            parameters: AnimatorParameters::new(),
            graph: None,
            graph_enabled: true,
            current_node: None,
            node_speed: 1.0,
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

    /// Advance both playheads (and any crossfade) by `dt` scaled seconds — the
    /// scale is `speed * node_speed`, the component multiplier stacked with the
    /// active graph node's own rate (#316).
    /// `clip_duration` is the current clip's length in seconds (non-positive when
    /// unknown): with [`loop_clip`](Self::loop_clip) set the playhead wraps modulo
    /// it, so the clip repeats seamlessly — a pure `rem_euclid`, deterministic at
    /// the fixed timestep. A non-looping clip runs past the end and the sampler
    /// holds its last frame, as before. The outgoing crossfade playhead never
    /// wraps — it stays independent, so a loop wrap mid-crossfade cannot glitch the
    /// fading-out pose. Ending the crossfade clears the previous clip so sampling
    /// falls back to a single pose.
    pub fn advance(&mut self, dt: f32, clip_duration: f32) {
        if !self.is_playing || self.freeze {
            return;
        }
        let step = dt * self.speed * self.node_speed;
        self.time += step;
        if self.loop_clip && clip_duration > 0.0 {
            self.time = self.time.rem_euclid(clip_duration);
        }
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
mod graph_state_tests;
#[cfg(test)]
mod parameters_tests;
#[cfg(test)]
mod tests;
