//! src/components/animator/sync.rs — one layer's sync-group state (#1896).
//!
//! What a [`Playback`] remembers so its motions keep step (Unreal's sync
//! groups, within a layer): the active node's group, a placement still to make
//! (entering a node of the same group starts the incoming motion at the
//! outgoing one's phase), whether the crossfade is a synced one (the lighter
//! motion follows the heavier each step), which leaf leads each blend tree, and
//! the travel a follower was re-matched back over. The phase arithmetic needs
//! the clips, so it lives in the runtime (`app::animation::sync`); this holds
//! only the state, and [`Playback::follow`], the one way a follower's
//! playhead moves.

use serde::{Deserialize, Serialize};

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
    /// against the direction of play: its events stay silent until it has
    /// travelled that far forward again, so none fires twice.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rewind: f32,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl SyncState {
    /// Nothing to remember: outside any group, nothing pending.
    pub fn is_idle(&self) -> bool {
        *self == Self::default()
    }

    /// The synced crossfade is over (or was dropped).
    pub(super) fn end_fade(&mut self) {
        self.following = false;
        self.previous_node = None;
        self.previous_leader = None;
        self.rewind = 0.0;
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
    /// it follows (#1896), by the shortest way round a looping playhead. A
    /// non-looping one follows until its end, then holds: a leader wrapping
    /// round its seam sends it on to its end, never back. `forward` is the leader's
    /// direction of play: travel against it fires no events and is owed back
    /// first ([`SyncState::rewind`]), so a re-match back and forth over an event
    /// fires it once.
    pub fn follow(&mut self, current: Playhead, target: f32, forward: bool) -> Step {
        let bounds = current.bounds(self.loop_clip);
        let held = self.ended && matches!(bounds, Bounds::Clamp(_));
        let mut step = self.move_current(current, |from| match bounds {
            _ if held => from,
            Bounds::Wrap(w) => from + wrap_delta(target - from, w),
            Bounds::Clamp(end) if forward && target < from => end,
            Bounds::Clamp(_) if !forward && target > from => 0.0,
            _ => target,
        });
        let dir = if forward { 1.0 } else { -1.0 };
        let along = (step.events.1 - step.events.0) * dir;
        if along < 0.0 {
            self.sync.rewind -= along;
            step.events.1 = step.events.0;
        } else {
            let owed = along.min(self.sync.rewind);
            self.sync.rewind -= owed;
            step.events.0 += owed * dir;
        }
        step
    }
}
