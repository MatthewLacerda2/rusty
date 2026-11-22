use serde::{Deserialize, Serialize};

use rusty_asset::animation_graph::GraphNode;

use super::playback::{Motion, Playback};
use super::playhead::{wrap_delta, Bounds, Playhead, Step};

/// The sync-group state of one playback. Idle (and not saved) outside a
/// group.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<SyncSource>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub following: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_leader: Option<String>,
    /// Playhead units re-matched against play, signed by its direction: silent until repaid.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rewind: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leaves: Vec<LeafTime>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous_leaves: Vec<LeafTime>,
}

/// One leaf clip of a marker-synced blend tree and where it was last sampled,
/// in clip seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeafTime {
    pub clip: String,
    pub time: f32,
}

pub fn stored_leaf(leaves: &[LeafTime], clip: &str) -> Option<f32> {
    leaves.iter().find(|l| l.clip == clip).map(|l| l.time)
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl SyncState {
    pub fn is_idle(&self) -> bool {
        *self == Self::default()
    }

    pub(super) fn begin_fade(&mut self) {
        *self = Self {
            previous_leader: self.leader.take(),
            previous_leaves: std::mem::take(&mut self.leaves),
            ..Self::default()
        };
    }

    pub(super) fn end_fade(&mut self) {
        // `rewind` stays owed: the motion plays on.
        self.following = false;
        self.previous_node = None;
        self.previous_leader = None;
        self.previous_leaves.clear();
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncSource {
    #[serde(default)]
    pub clip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default)]
    pub time: f32,
    #[serde(default)]
    pub looping: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
}

impl SyncSource {
    pub fn of(playback: &Playback) -> Self {
        Self {
            clip: playback.current_clip.clone(),
            tree: playback.current_tree.clone(),
            node: playback.current_node.clone(),
            time: playback.time,
            looping: playback.loop_clip,
            leader: playback.sync.leader.clone(),
        }
    }

    pub fn motion(&self) -> Motion<'_> {
        match &self.tree {
            Some(node) => Motion::Tree(node),
            None => Motion::Clip(&self.clip),
        }
    }
}

impl Playback {
    pub fn follow(&mut self, current: Playhead, target: f32, forward: bool) -> Step {
        let bounds = current.bounds(self.loop_clip);
        let mut step = self.move_current(current, |from| match bounds {
            Bounds::Wrap(w) => from + wrap_delta(target - from, w),
            Bounds::Clamp(end) => follow_clamped(from, target, end, forward),
            Bounds::Open => target,
        });
        let dir = if forward { 1.0 } else { -1.0 };
        let along = (step.events.1 - step.events.0) * dir;
        if along < 0.0 {
            self.drop_flipped_rewind(dir);
            self.sync.rewind -= along * dir;
            step.events.1 = step.events.0;
            return step;
        }
        self.repay(step, forward)
    }

    /// Forget travel owed against the other direction of play: once play
    /// turns round, what was re-matched back is ahead again, not owed.
    fn drop_flipped_rewind(&mut self, dir: f32) {
        if self.sync.rewind * dir < 0.0 {
            self.sync.rewind = 0.0;
        }
    }

    pub fn repay(&mut self, mut step: Step, forward: bool) -> Step {
        let dir = if forward { 1.0 } else { -1.0 };
        let along = (step.events.1 - step.events.0) * dir;
        if along <= 0.0 {
            return step; // nothing travelled: a pause turns nothing round
        }
        self.drop_flipped_rewind(dir);
        let owed = along.min(self.sync.rewind * dir);
        if owed > 0.0 {
            self.sync.rewind -= owed * dir;
            step.events.0 += owed * dir;
        }
        step
    }

    pub(super) fn sync_source(&self, node: &GraphNode) -> Option<SyncSource> {
        let synced = node.sync_group.is_some()
            && node.sync_group == self.sync.group
            && self.current_node.as_deref() != Some(node.name.as_str());
        // A placement still pending wins, so a motion never placed is not taken as the phase.
        synced.then(|| {
            self.sync
                .from
                .clone()
                .unwrap_or_else(|| SyncSource::of(self))
        })
    }

    pub(super) fn start_sync(
        &mut self,
        node: &GraphNode,
        previous: Option<String>,
        source: Option<SyncSource>,
    ) {
        self.sync.group = node.sync_group.clone();
        let Some(source) = source else {
            return;
        };
        self.sync.following = self.is_crossfading();
        if self.sync.following {
            self.sync.previous_node = previous;
        }
        self.sync.from = Some(source);
    }
}

pub fn follow_clamped(from: f32, target: f32, end: f32, forward: bool) -> f32 {
    let seam = (target - from).abs() > end * 0.5;
    match () {
        _ if forward && from >= end => from,
        _ if !forward && from <= 0.0 => from,
        _ if forward && target < from && seam => end,
        _ if !forward && target > from && seam => 0.0,
        _ => target,
    }
}
