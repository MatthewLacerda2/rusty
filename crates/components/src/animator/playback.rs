use serde::{Deserialize, Serialize};

use rusty_asset::animation_graph::GraphNode;

use super::playhead::{finite, Bounds, Playhead, Step};
use super::states::NodeStates;
use super::sync::SyncState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion<'a> {
    Clip(&'a str),
    Tree(&'a str),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    #[serde(default)]
    pub current_clip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_tree: Option<String>,
    /// Playhead: clip seconds, or a tree's normalized phase.
    #[serde(default)]
    pub time: f32,
    #[serde(default)]
    pub loop_clip: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ended: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_clip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_tree: Option<String>,
    /// Playhead of the outgoing motion; it keeps running, never wraps.
    #[serde(default)]
    pub previous_time: f32,
    #[serde(default)]
    pub previous_loop: bool,
    /// Seconds elapsed in the active crossfade.
    #[serde(default)]
    pub crossfade_elapsed: f32,
    /// Total crossfade length in seconds; `0.0` means no crossfade is active.
    #[serde(default)]
    pub crossfade_duration: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_node: Option<String>,
    #[serde(
        default = "default_node_speed",
        skip_serializing_if = "is_default_speed"
    )]
    pub node_speed: f32,
    #[serde(default, skip_serializing_if = "SyncState::is_idle")]
    pub sync: SyncState,
    #[serde(skip)]
    pub states: NodeStates,
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
            sync: SyncState::default(),
            states: NodeStates::default(),
        }
    }
}

impl Playback {
    pub fn motion(&self) -> Motion<'_> {
        match &self.current_tree {
            Some(node) => Motion::Tree(node),
            None => Motion::Clip(&self.current_clip),
        }
    }

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

    pub fn play(&mut self, motion: Motion<'_>) {
        self.set_current(motion);
        self.time = 0.0;
        self.end_crossfade();
        self.sync = SyncState::default();
    }

    pub fn crossfade(&mut self, motion: Motion<'_>, duration: f32) {
        if duration <= 0.0 || motion == self.motion() {
            self.play(motion);
            return;
        }
        self.previous_clip = Some(std::mem::take(&mut self.current_clip));
        self.previous_tree = self.current_tree.take();
        self.previous_time = self.time;
        self.previous_loop = self.loop_clip;
        self.states.previous_node = None; // `enter_node` names the node it leaves
        self.sync.begin_fade();
        self.set_current(motion);
        self.time = 0.0;
        self.crossfade_elapsed = 0.0;
        self.crossfade_duration = duration;
    }

    pub fn enter_node(&mut self, node: &GraphNode, blend: f32) {
        let motion = match node.blend_tree {
            Some(_) => Motion::Tree(&node.name),
            None => Motion::Clip(&node.clip),
        };
        let source = self.sync_source(node);
        self.crossfade(motion, blend);
        self.loop_clip = node.is_loop;
        self.node_speed = node.speed.unwrap_or(1.0);
        let previous = self.current_node.replace(node.name.clone());
        let fading = self.is_crossfading();
        self.states.entered(previous.clone(), fading);
        self.start_sync(node, previous, source);
    }

    pub fn crossfade_weight(&self) -> f32 {
        if self.crossfade_duration <= 0.0 {
            return 1.0;
        }
        (self.crossfade_elapsed / self.crossfade_duration).clamp(0.0, 1.0)
    }

    pub fn is_crossfading(&self) -> bool {
        (self.previous_clip.is_some() || self.previous_tree.is_some())
            && self.crossfade_duration > 0.0
    }

    pub fn advance(
        &mut self,
        dt: f32,
        speed: f32,
        current: Playhead,
        previous_per_second: f32,
    ) -> Step {
        let step = self.scaled(dt, speed);
        let moved = self.advance_current(step, current);
        self.advance_previous(dt, step, previous_per_second);
        moved
    }

    pub fn scaled(&self, dt: f32, speed: f32) -> f32 {
        finite(dt * speed * self.node_speed)
    }

    pub fn advance_current(&mut self, step: f32, current: Playhead) -> Step {
        self.move_current(current, |from| {
            finite(from + finite(step * current.per_second))
        })
    }

    /// The crossfade runs on unscaled `dt`, so speed 0 still completes it.
    pub fn advance_previous(&mut self, dt: f32, step: f32, per_second: f32) {
        if self.is_crossfading() {
            self.previous_time = finite(self.previous_time + finite(step * per_second));
            self.crossfade_elapsed += dt;
            if self.crossfade_elapsed >= self.crossfade_duration {
                self.end_crossfade();
            }
        }
    }

    pub(super) fn move_current(&mut self, current: Playhead, to: impl FnOnce(f32) -> f32) -> Step {
        let bounds = current.bounds(self.loop_clip);
        // Past the end before any clamp: the clip's length was unknown while it
        // ran on and is known now.
        let late = matches!(bounds, Bounds::Clamp(end) if self.time > end);
        let from = finite(match bounds {
            Bounds::Clamp(end) => self.time.clamp(0.0, end),
            _ => self.time,
        });
        let to = finite(to(from));
        self.time = bounds.apply(to);
        // A looping clip never ends, and an unknown length says nothing about the end: `ended`
        // holds, so no second `End`.
        let Bounds::Clamp(end) = bounds else {
            return Step::travel(from, to);
        };
        let resting = self.ended && from >= end;
        let end_now = !self.ended && (late || (self.time >= end && to >= from));
        self.ended = (self.ended || end_now) && self.time >= end;
        let pinned = late || (resting && to > from) || (from <= 0.0 && to < from);
        let span = if pinned { (from, from) } else { (from, to) };
        Step {
            span,
            events: span,
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
        self.states.previous_node = None;
        self.sync.end_fade();
    }
}
