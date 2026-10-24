//! src/components/animator/playback.rs — one layer's playback state (#457).
//!
//! What one state machine is playing: its motion (a clip, or the blend tree of a
//! graph node), the playhead, an outgoing motion while a crossfade blends, and the
//! active graph node. The base layer's lives flattened at the top of
//! [`AnimatorComponent`](super::AnimatorComponent) (a one-layer animator
//! reads as it always has); every extra layer has its own in a [`LayerState`](super::LayerState).
//!
//! A clip's playhead is in clip seconds. A blend tree's is **normalized** (0..1 is
//! one cycle), because its children have different lengths and are sampled at the
//! same phase — that is what keeps a walk and a run blended together in step.

use serde::{Deserialize, Serialize};

use crate::asset::animation_graph::GraphNode;

use super::playhead::{finite, Bounds, Playhead};

/// What a playback plays: a clip by name, or the blend tree of the named graph
/// node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion<'a> {
    Clip(&'a str),
    Tree(&'a str),
}

/// What one [`Playback::advance`] did to the current playhead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The playhead's travel, `(from, to)` before any wrap or clamp — what
    /// animation events (#459) are crossed against. A clamped playhead already
    /// resting on the end it is pushed against travels nothing, so a marker there
    /// fires once per pass, not every step; nor does the step a clip's length
    /// becomes known, since its unresolved run already crossed the markers (#1386).
    pub span: (f32, f32),
    /// A non-looping playhead reached its end this step, playing forwards or
    /// standing still: once per pass, including the step a clip's length became
    /// known with the playhead already past it, and a zero-length clip's first
    /// (#1386). What fires the automatic `End` event.
    pub end: bool,
}

impl Step {
    fn travel(from: f32, to: f32) -> Self {
        Self {
            span: (from, to),
            end: false,
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
    /// Loop the current motion: the playhead wraps at its end; without it, it is
    /// clamped to the length (#1362), so reversing moves back at once. Persists
    /// across `play`/`crossfade`, like `speed` (#313).
    #[serde(default)]
    pub loop_clip: bool,
    /// A non-looping playhead has reached its end and rests there, so its next
    /// steps don't fire `End` again; cleared when it leaves the end (#1386).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ended: bool,
    /// The clip being faded *out* during a crossfade. `None` outside one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_clip: Option<String>,
    /// The graph node whose blend tree is being faded out (#457).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_tree: Option<String>,
    /// Playhead of the outgoing motion; it keeps running, never wraps.
    #[serde(default)]
    pub previous_time: f32,
    /// The outgoing motion's `loop_clip`, so it samples the way it played.
    #[serde(default)]
    pub previous_loop: bool,
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
            ended: false,
            previous_clip: None,
            previous_tree: None,
            previous_time: 0.0,
            previous_loop: false,
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
        self.previous_loop = self.loop_clip;
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
    /// one by `current`, within its [`Bounds`] under `loop_clip` — wrapped when
    /// looping, otherwise clamped to `[0, wrap]` (#1362), so reversing from past an
    /// end moves back at once; the outgoing one at `previous_per_second`, never
    /// wrapping. Both stay finite whatever the speed. The crossfade itself runs on
    /// unscaled `dt` and, once done, drops the outgoing motion.
    pub fn advance(
        &mut self,
        dt: f32,
        speed: f32,
        current: Playhead,
        previous_per_second: f32,
    ) -> Step {
        let step = finite(dt * speed * self.node_speed);
        let bounds = current.bounds(self.loop_clip);
        // Past the end before any clamp: the clip's length was unknown while it
        // ran on and is known now (#1386).
        let late = matches!(bounds, Bounds::Clamp(end) if self.time > end);
        let from = finite(match bounds {
            Bounds::Clamp(end) => self.time.clamp(0.0, end),
            _ => self.time,
        });
        let to = finite(from + finite(step * current.per_second));
        self.time = bounds.apply(to);
        if self.is_crossfading() {
            self.previous_time = finite(self.previous_time + finite(step * previous_per_second));
            self.crossfade_elapsed += dt;
            if self.crossfade_elapsed >= self.crossfade_duration {
                self.end_crossfade();
            }
        }
        // A length that isn't known (or doesn't clamp) says nothing about the
        // end: `ended` holds, so a clip that stops resolving for a step doesn't
        // end (and fire its end markers) again once it resolves.
        let Bounds::Clamp(end) = bounds else {
            return Step::travel(from, to);
        };
        let resting = self.ended && from >= end;
        let end_now = !self.ended && (late || (self.time >= end && to >= from));
        self.ended = (self.ended || end_now) && self.time >= end;
        // Pinned: a push against the end it rests on, a reverse push against the
        // start, and the late step, whose travel the unresolved run already made,
        // so a marker it crossed isn't replayed where it clamps onto the end.
        let pinned = late || (resting && to > from) || (from <= 0.0 && to < from);
        Step {
            span: if pinned { (from, from) } else { (from, to) },
            end: end_now,
        }
    }

    fn set_current(&mut self, motion: Motion<'_>) {
        self.ended = false;
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
