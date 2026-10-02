//! src/components/animator/playback.rs — one layer's playback state (#457).
//!
//! What one state machine is playing: its motion (a clip, or the blend tree of a
//! graph node), the playhead, an outgoing motion while a crossfade blends, and the
//! active graph node. The base layer's lives flattened at the top of
//! [`AnimatorComponent`](super::AnimatorComponent) (so pre-#457 scenes load
//! unchanged); every extra layer has its own in a [`LayerState`](super::LayerState).
//!
//! A clip's playhead is in clip seconds. A blend tree's is **normalized** (0..1 is
//! one cycle), because its children have different lengths and are sampled at the
//! same phase — that is what keeps a walk and a run blended together in step.

use serde::{Deserialize, Serialize};

use crate::asset::animation_graph::GraphNode;

/// What a playback plays: a clip by name, or the blend tree of the named graph
/// node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion<'a> {
    Clip(&'a str),
    Tree(&'a str),
}

/// How one motion's playhead moves this step: playhead units per second of
/// play (`1` for a clip, `1 / blended length` for a tree) and the length it wraps
/// at when looping (the clip's duration, `1` for a tree; non-positive: unknown,
/// never wrap).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playhead {
    pub per_second: f32,
    pub wrap: f32,
}

impl Playhead {
    /// A clip's playhead: seconds, wrapping at `duration`.
    pub fn clip(duration: f32) -> Self {
        Self {
            per_second: 1.0,
            wrap: duration,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    /// Name of the clip playing (matched against the mesh's clip list). Empty
    /// while a blend tree plays.
    #[serde(default)]
    pub current_clip: String,
    /// The graph node whose blend tree plays, if one does (#457).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_tree: Option<String>,
    /// Playhead: clip seconds, or a tree's normalized phase.
    #[serde(default)]
    pub time: f32,
    /// Loop the current motion: the playhead wraps instead of running past the
    /// end (where the sampler holds the last frame). Persists across
    /// `play`/`crossfade`, like `speed` (#313).
    #[serde(default)]
    pub loop_clip: bool,
    /// The clip being faded *out* during a crossfade. `None` outside one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_clip: Option<String>,
    /// The graph node whose blend tree is being faded out (#457).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_tree: Option<String>,
    /// Playhead of the outgoing motion; it keeps running, never wraps.
    #[serde(default)]
    pub previous_time: f32,
    /// Seconds elapsed in the active crossfade.
    #[serde(default)]
    pub crossfade_elapsed: f32,
    /// Total crossfade length in seconds; `0.0` means no crossfade is active.
    #[serde(default)]
    pub crossfade_duration: f32,
    /// Name of the active graph node (#316). `None` until the evaluator binds the
    /// graph, or after the graph reference changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_node: Option<String>,
    /// The active node's playback-rate multiplier, stacked on the component's
    /// `speed`. `1.0` outside a graph.
    #[serde(
        default = "default_node_speed",
        skip_serializing_if = "is_default_speed"
    )]
    pub node_speed: f32,
}

fn default_node_speed() -> f32 {
    1.0
}

fn is_default_speed(v: &f32) -> bool {
    *v == 1.0
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            current_clip: String::new(),
            current_tree: None,
            time: 0.0,
            loop_clip: false,
            previous_clip: None,
            previous_tree: None,
            previous_time: 0.0,
            crossfade_elapsed: 0.0,
            crossfade_duration: 0.0,
            current_node: None,
            node_speed: 1.0,
        }
    }
}

impl Playback {
    /// The motion playing now.
    pub fn motion(&self) -> Motion<'_> {
        match &self.current_tree {
            Some(node) => Motion::Tree(node),
            None => Motion::Clip(&self.current_clip),
        }
    }

    /// The motion being faded out, while a crossfade blends.
    pub fn previous_motion(&self) -> Option<Motion<'_>> {
        if !self.is_crossfading() {
            return None;
        }
        match (&self.previous_tree, &self.previous_clip) {
            (Some(node), _) => Some(Motion::Tree(node)),
            (None, Some(clip)) => Some(Motion::Clip(clip)),
            (None, None) => None,
        }
    }

    /// Hard-cut to `motion` from its start, discarding any crossfade.
    pub fn play(&mut self, motion: Motion<'_>) {
        self.set_current(motion);
        self.time = 0.0;
        self.end_crossfade();
    }

    /// Crossfade into `motion` over `duration` seconds. A non-positive duration,
    /// or a fade into the motion already playing, degrades to [`play`](Self::play).
    pub fn crossfade(&mut self, motion: Motion<'_>, duration: f32) {
        if duration <= 0.0 || motion == self.motion() {
            self.play(motion);
            return;
        }
        self.previous_clip = Some(std::mem::take(&mut self.current_clip));
        self.previous_tree = self.current_tree.take();
        self.previous_time = self.time;
        self.set_current(motion);
        self.time = 0.0;
        self.crossfade_elapsed = 0.0;
        self.crossfade_duration = duration;
    }

    /// Enter graph `node`: crossfade into its motion over `blend` seconds, adopt
    /// its loop flag and per-node speed, and make it the active state.
    pub fn enter_node(&mut self, node: &GraphNode, blend: f32) {
        let motion = match node.blend_tree {
            Some(_) => Motion::Tree(&node.name),
            None => Motion::Clip(&node.clip),
        };
        self.crossfade(motion, blend);
        self.loop_clip = node.is_loop;
        self.node_speed = node.speed.unwrap_or(1.0);
        self.current_node = Some(node.name.clone());
    }

    /// The crossfade weight in `[0, 1]`: 0 fully on the outgoing motion, 1 fully
    /// on the current one. `1.0` when no crossfade is active.
    pub fn crossfade_weight(&self) -> f32 {
        if self.crossfade_duration <= 0.0 {
            return 1.0;
        }
        (self.crossfade_elapsed / self.crossfade_duration).clamp(0.0, 1.0)
    }

    /// True while a crossfade is blending.
    pub fn is_crossfading(&self) -> bool {
        (self.previous_clip.is_some() || self.previous_tree.is_some())
            && self.crossfade_duration > 0.0
    }

    /// Advance the playheads by `dt` seconds at `speed × node_speed`: the current
    /// one by `current`, wrapping when looping (a pure `rem_euclid`, so the fixed
    /// timestep stays deterministic); the outgoing one at `previous_per_second`,
    /// never wrapping. The crossfade itself runs on unscaled `dt` and, once done,
    /// drops the outgoing motion. Returns the current playhead's travel this step,
    /// `(from, to)` before any wrap — what animation events (#459) are crossed
    /// against.
    pub fn advance(
        &mut self,
        dt: f32,
        speed: f32,
        current: Playhead,
        previous_per_second: f32,
    ) -> (f32, f32) {
        let step = dt * speed * self.node_speed;
        let from = self.time;
        self.time += step * current.per_second;
        let to = self.time;
        if self.loop_clip && current.wrap > 0.0 {
            self.time = self.time.rem_euclid(current.wrap);
        }
        if self.is_crossfading() {
            self.previous_time += step * previous_per_second;
            self.crossfade_elapsed += dt;
            if self.crossfade_elapsed >= self.crossfade_duration {
                self.end_crossfade();
            }
        }
        (from, to)
    }

    fn set_current(&mut self, motion: Motion<'_>) {
        match motion {
            Motion::Clip(clip) => {
                self.current_clip = clip.to_string();
                self.current_tree = None;
            }
            Motion::Tree(node) => {
                self.current_clip.clear();
                self.current_tree = Some(node.to_string());
            }
        }
    }

    fn end_crossfade(&mut self) {
        self.previous_clip = None;
        self.previous_tree = None;
        self.crossfade_elapsed = 0.0;
        self.crossfade_duration = 0.0;
    }
}
