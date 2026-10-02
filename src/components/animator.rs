//! src/components/animator.rs — Animator component
//!
//! The runtime state of an entity's skeletal animation: which clip is playing, the
//! playhead, speed, and a minimal current-state machine for crossfades (#80). The
//! clips themselves live on the entity's skinned `MeshComponent` (rehydrated from
//! the imported source, never serialized); this component only references one by
//! name and tracks where the playhead is. Since #457 an animator plays one
//! [`Playback`] per graph layer: the base layer's, plus one [`LayerState`] per
//! extra layer. Unity analog: `Animator`.

use serde::{Deserialize, Serialize};

mod graph_state;
mod parameters;
mod playback;
pub use parameters::{AnimatorParameter, AnimatorParameters};
pub use playback::{Motion, Playback, Playhead};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorComponent {
    /// The base layer's playback: clip or blend tree, playhead, crossfade and
    /// active graph node. Flattened, so its fields sit at the top of the saved
    /// component exactly as before layers existed (#457).
    #[serde(flatten)]
    pub base: Playback,
    /// Playback rate multiplier applied to the fixed timestep, every layer.
    pub speed: f32,
    pub is_playing: bool,
    /// Pause hold (`Animator.Pause`/`Resume`, the editor's Freeze toggle): freezes
    /// every playhead without clearing `is_playing`, so the pose holds and
    /// playback resumes from the same frame.
    pub freeze: bool,
    /// Typed graph parameters (#314): the named `Bool`/`Float`/`Int`/`Trigger`
    /// values scripts set (`Animator.Set*`) and the `AnimationGraph`'s edge
    /// conditions and blend trees read. Values live here and serialize with the
    /// entity; declarations belong to the graph asset. Name-sorted (`BTreeMap`) so
    /// the evaluator's fixed-step walk is deterministic. Shared by every layer.
    #[serde(default, skip_serializing_if = "AnimatorParameters::is_empty")]
    pub parameters: AnimatorParameters,
    /// Path to the `AnimationGraph` asset (`guard.animgraph`, #315) driving this
    /// animator — a path-based reference, exactly like a mesh references its source
    /// file, so the scene stores the reference and the graph is rehydrated on load.
    /// `None` means no graph: the animator only does what scripts tell it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
    /// Auto-evaluate the referenced graph each fixed step (#316). On by default;
    /// off (`Animator.SetGraphEnabled(id, false)`), the graph is inert data and
    /// the animator stays under direct `Play`/`Crossfade` control.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub graph_enabled: bool,
    /// The graph's extra layers' runtime state (#457), index `i` for layer `i + 1`.
    /// Filled when the evaluator binds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LayerState>,
}

/// One extra layer's runtime state (#457).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerState {
    /// The layer's name in the graph, refreshed by the evaluator — how a script
    /// addresses the layer by name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The layer's live weight in `[0, 1]`: the graph's authored weight when the
    /// layer binds, then whatever `Animator.SetLayerWeight` sets.
    pub weight: f32,
    #[serde(flatten)]
    pub playback: Playback,
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

impl Default for AnimatorComponent {
    fn default() -> Self {
        Self {
            base: Playback::default(),
            speed: 1.0,
            is_playing: false,
            freeze: false,
            parameters: AnimatorParameters::new(),
            graph: None,
            graph_enabled: true,
            layers: Vec::new(),
        }
    }
}

impl AnimatorComponent {
    /// Start `clip` on the base layer immediately, discarding any crossfade — a
    /// hard cut from the current pose, from the clip's start.
    pub fn play(&mut self, clip: String) {
        self.base.play(Motion::Clip(&clip));
        self.run();
    }

    /// Crossfade the base layer into `clip` over `duration` seconds. A
    /// non-positive `duration`, or a fade into the already-current clip, degrades
    /// to a plain [`play`](Self::play).
    pub fn crossfade(&mut self, clip: String, duration: f32) {
        self.base.crossfade(Motion::Clip(&clip), duration);
        self.run();
    }

    /// The base layer's crossfade weight (see [`Playback::crossfade_weight`]).
    pub fn crossfade_weight(&self) -> f32 {
        self.base.crossfade_weight()
    }

    /// True while the base layer is crossfading.
    pub fn is_crossfading(&self) -> bool {
        self.base.is_crossfading()
    }

    /// True when the playheads move this step: playing and not paused.
    pub fn is_running(&self) -> bool {
        self.is_playing && !self.freeze
    }

    /// Advance the base layer's clip playhead by `dt` (scaled by `speed` and the
    /// node's rate), wrapping at `clip_duration` when looping — the single-clip
    /// shorthand of [`Playback::advance`] (the `animate` system advances every
    /// layer, blend trees included, through that directly).
    pub fn advance(&mut self, dt: f32, clip_duration: f32) {
        if self.is_running() {
            let speed = self.speed;
            self.base
                .advance(dt, speed, Playhead::clip(clip_duration), 1.0);
        }
    }

    fn run(&mut self) {
        self.is_playing = true;
        self.freeze = false;
    }
}

#[cfg(test)]
mod graph_state_tests;
#[cfg(test)]
mod parameters_tests;
#[cfg(test)]
mod tests;
