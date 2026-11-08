//! src/components/animator/sync.rs — one layer's sync-group state (#1896).
//!
//! What a [`Playback`] remembers so its motions keep step (Unreal's sync
//! groups, within a layer): the active node's group, a placement still to make
//! (entering a node of the same group starts the incoming motion at the
//! outgoing one's phase), whether the crossfade is a synced one (the lighter
//! motion follows the heavier each step), which leaf leads each blend tree and
//! where each of its leaves was, and the travel a follower was re-matched back
//! over. The phase arithmetic needs
//! the clips, so it lives in the runtime (`app::animation::sync`); this holds
//! only the state, and [`Playback::follow`], the one way a follower's
//! playhead moves.

use serde::{Deserialize, Serialize};

use crate::asset::animation_graph::GraphNode;

use super::playback::{Motion, Playback};
use super::playhead::{wrap_delta, Bounds, Playhead, Step};

/// The sync-group state of one playback. Idle (and not saved) outside a
/// group.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncState {
    /// The active graph node's sync group, if it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The motion to match the current one's phase to on the next step: what
    /// was playing when a node of the same group was entered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<SyncSource>,
    /// The crossfade is between two nodes of one group: each step the lighter
    /// motion follows the heavier one's phase.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub following: bool,
    /// The graph node being faded out by a synced crossfade (its clip's
    /// markers are the node's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_node: Option<String>,
    /// The leaf clip leading the current blend tree's marker sync, so a change
    /// of leader carries the phase over instead of jumping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
    /// The same for the blend tree being faded out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_leader: Option<String>,
    /// How far, in playhead units, a following current motion was re-matched
    /// against the direction of play, signed by that direction (positive while
    /// playing forward): its events stay silent until it has travelled that
    /// far along it again, so none fires twice. A flip of the direction of
    /// play drops it, so the first crossing the other way fires.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rewind: f32,
    /// Where each leaf of a marker-synced current tree was last placed, so a
    /// follower with a repeated pair of markers stays on the occurrence it is
    /// on instead of jumping to the one nearest its normalized time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leaves: Vec<LeafTime>,
    /// The same for the blend tree being faded out.
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

/// `clip`'s time in `leaves`, if it is there.
pub fn stored_leaf(leaves: &[LeafTime], clip: &str) -> Option<f32> {
    leaves.iter().find(|l| l.clip == clip).map(|l| l.time)
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl SyncState {
    /// Nothing to remember: outside any group, nothing pending.
    pub fn is_idle(&self) -> bool {
        *self == Self::default()
    }

    /// A new crossfade starts: nothing synced yet, and the motion fading out
    /// keeps its tree's leader and where its leaves were.
    pub(super) fn begin_fade(&mut self) {
        *self = Self {
            previous_leader: self.leader.take(),
            previous_leaves: std::mem::take(&mut self.leaves),
            ..Self::default()
        };
    }

    /// The synced crossfade is over (or was dropped). Travel still owed
    /// ([`rewind`](Self::rewind)) stays owed: the motion plays on.
    pub(super) fn end_fade(&mut self) {
        self.following = false;
        self.previous_node = None;
        self.previous_leader = None;
        self.previous_leaves.clear();
    }
}

/// A motion as it was when a node of its group was entered: what plays, from
/// which node, where its playhead stood and how it bounds it.
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
    /// What `playback` plays now.
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
    /// Move the current playhead to `target`, a phase re-matched to the motion
    /// it follows (#1896), by the shortest way round a looping playhead; a
    /// non-looping one goes by [`follow_clamped`]. `forward` is the leader's
    /// direction of play: travel against it fires no events and is owed back
    /// first ([`SyncState::rewind`]), so a re-match back and forth over an event
    /// fires it once.
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

    /// `step` with the part of its travel still owed to an earlier re-match
    /// back ([`SyncState::rewind`]) silenced: whether the motion follows,
    /// leads or plays on alone, an event it was moved back over fires once.
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

    /// What entering `node` syncs to: the motion playing now when `node` is
    /// another node of the active sync group, or the placement still pending
    /// from an earlier entry this step (so a motion never placed is not taken
    /// as the phase). Re-entering the current node restarts it, as ever.
    pub(super) fn sync_source(&self, node: &GraphNode) -> Option<SyncSource> {
        let synced = node.sync_group.is_some()
            && node.sync_group == self.sync.group
            && self.current_node.as_deref() != Some(node.name.as_str());
        synced.then(|| {
            self.sync
                .from
                .clone()
                .unwrap_or_else(|| SyncSource::of(self))
        })
    }

    /// Adopt `node`'s sync group after entering it from `previous`, syncing to
    /// `source` when there is one.
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

/// Where a non-looping follower of length `end` at `from` goes when its
/// leader re-matches it to `target` (#1896): there, except that one resting
/// on its end (its start, playing backwards) stays, and a jump against `forward` of over half its length is
/// the leader wrapping round its seam, which sends it on to its end (back to
/// its start, playing backwards), never back across itself.
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
