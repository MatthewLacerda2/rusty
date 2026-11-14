//! crates/app/src/animation/sync/ — keeping motions in step: sync groups and sync
//! markers (#1896), Unreal's model.
//!
//! A motion's phase is read off its **lead clip**: a clip node's clip, or a
//! blend tree's dominant leaf (the one whose events fire,
//! [`dominant_leaf`]). With sync markers the phase is a [`SyncPos`]: which
//! pair of markers the clip is between and how far (`FootL`→`FootR`, 40 %);
//! another clip is put at the same place between its own pair of those names.
//! A clip without the pair takes the normalized phase instead (Unreal: no
//! common markers, length-based sync).
//!
//! Three users:
//! - **entering a node of the same group** places the incoming motion at the
//!   outgoing one's phase ([`MotionContext::place_pending`]), with no travel,
//!   so no event fires for the jump;
//! - **a synced crossfade** moves the heavier motion (the incoming one on a
//!   tie) by its own rate and re-matches the lighter one to it each step
//!   ([`MotionContext::advance_synced`]); a following incoming motion still
//!   crosses its own events once each (`Playback::follow`);
//! - **a blend tree whose every leaf carries markers** samples each leaf at
//!   the dominant leaf's place between markers ([`TreeSync`]), and carries
//!   its phase across a change of dominant leaf ([`MotionContext::rebase`]);
//!   each follower leaf is kept on the occurrence of its markers nearest
//!   where it was (`SyncState::leaves`, [`MotionContext::track_leaves`]), so a
//!   leaf repeating the leader's pair never jumps a stride.
//!
//! Pure functions of (graph, clips, parameters, playheads): deterministic.

use rusty_asset::anim_data::AnimationClip;
use rusty_asset::animation_graph::{BlendTree, SyncMarker, MAX_TREE_DEPTH};
use rusty_components::{stored_leaf, LeafTime, Motion, Playhead};

use super::blend_tree::dominant_leaf;
use super::motion::MotionContext;
pub use markers::SyncPos;
use markers::{place, position};

mod fade;
mod markers;
mod place;

pub(super) use place::place_pending;

/// The clip a motion keeps step by, its markers, and whether its playhead is a
/// tree's normalized phase (one unit is the clip's length) or clip seconds.
#[derive(Clone, Copy)]
pub(super) struct Lead<'a> {
    pub clip: &'a AnimationClip,
    pub markers: &'a [SyncMarker],
    pub tree: bool,
}

impl<'a> Lead<'a> {
    /// Clip seconds per playhead unit.
    fn scale(&self) -> f32 {
        if self.tree {
            self.clip.duration
        } else {
            1.0
        }
    }

    /// The playhead's length: one cycle of a tree, the clip's duration.
    pub fn wrap(&self) -> f32 {
        if self.tree {
            1.0
        } else {
            self.clip.duration
        }
    }

    /// Where the motion is with its playhead at `time`, bounded the way it
    /// plays.
    pub fn position(&self, time: f32, looping: bool) -> SyncPos<'a> {
        let bounds = Playhead::clip(self.wrap()).bounds(looping);
        let t = bounds.apply(time) * self.scale();
        position(self.markers, self.clip.duration, t, looping)
    }

    /// The playhead time that puts the motion at `pos`, the nearest such to
    /// `near` (a playhead time).
    pub fn place(&self, pos: SyncPos, near: f32, looping: bool) -> f32 {
        let scale = self.scale();
        let t = place(self.markers, self.clip.duration, pos, near * scale, looping);
        if scale > 0.0 {
            t / scale
        } else {
            0.0
        }
    }
}

/// A marker-synced blend tree's leader at one phase: each other leaf is
/// sampled where [`TreeSync::leaf_time`] puts it.
pub(super) struct TreeSync<'a> {
    leader: &'a str,
    pos: SyncPos<'a>,
    looping: bool,
}

impl TreeSync<'_> {
    /// Where leaf `clip` with `markers` is sampled at tree phase `phase`
    /// (bounded): the leader at `phase × length`, every other leaf at the
    /// leader's place between its markers, on the occurrence of that pair
    /// nearest `near` (where the leaf was, [`SyncState::leaves`]; its
    /// normalized time when it has no place yet).
    ///
    /// [`SyncState::leaves`]: rusty_components::SyncState::leaves
    pub fn leaf_time(
        &self,
        clip: &AnimationClip,
        markers: &[SyncMarker],
        phase: f32,
        near: Option<f32>,
    ) -> f32 {
        let natural = phase * clip.duration;
        if clip.name == self.leader {
            return natural;
        }
        let near = near.unwrap_or(natural);
        place(markers, clip.duration, self.pos, near, self.looping)
    }
}

impl MotionContext<'_> {
    /// `motion`'s lead clip: a clip node's clip with the markers of `node`
    /// (when that node plays it), or a tree's leaf `leader` names (when it is
    /// one of its leaves) or else its dominant leaf, with markers only when
    /// every leaf of the tree has them. `None` when nothing resolves.
    pub(super) fn lead(
        &self,
        motion: Motion<'_>,
        node: Option<&str>,
        leader: Option<&str>,
    ) -> Option<Lead<'_>> {
        match motion {
            Motion::Clip(name) => {
                let clip = self.clip(name)?;
                let node = node.and_then(|n| self.machine?.node(n));
                let node = node.filter(|n| n.blend_tree.is_none() && n.clip == name);
                let markers = node.map_or(&[][..], |n| &n.sync_markers[..]);
                Some(Lead {
                    clip,
                    markers,
                    tree: false,
                })
            }
            Motion::Tree(name) => {
                let tree = self.machine?.node(name)?.blend_tree.as_ref()?;
                let named = leader.and_then(|l| leaf_markers(tree, l, 1).map(|m| (l, m)));
                let named = named.filter(|(l, _)| self.clip(l).is_some());
                let (clip, markers) = match named {
                    Some(pair) => pair,
                    None => {
                        let resolves = |clip: &str| self.clip(clip).is_some();
                        let leaf = dominant_leaf(tree, self.parameters, &resolves)?;
                        (leaf.clip, leaf.sync_markers)
                    }
                };
                let markers = if tree.marker_synced() { markers } else { &[] };
                Some(Lead {
                    clip: self.clip(clip)?,
                    markers,
                    tree: true,
                })
            }
        }
    }

    /// Whether tree `node` of this layer has sync markers on every leaf.
    fn marker_synced(&self, node: &str) -> bool {
        let tree = self.machine.and_then(|m| m.node(node)?.blend_tree.as_ref());
        tree.is_some_and(BlendTree::marker_synced)
    }

    /// The marker sync of tree `node` at `phase`, when every leaf carries
    /// markers.
    pub(super) fn tree_sync(&self, node: &str, phase: f32, looping: bool) -> Option<TreeSync<'_>> {
        if !self.marker_synced(node) {
            return None; // checked first: a tree without markers pays no leader search
        }
        let lead = self.lead(Motion::Tree(node), None, None)?;
        if lead.markers.is_empty() {
            return None;
        }
        Some(TreeSync {
            leader: &lead.clip.name,
            pos: position(
                lead.markers,
                lead.clip.duration,
                phase * lead.clip.duration,
                looping,
            ),
            looping,
        })
    }

    /// Carry a marker-synced tree's phase across a change of dominant leaf:
    /// the new leader starts where it was being sampled as a follower, so the
    /// pose doesn't jump. `stored` is the leader last seen; a clip or an
    /// unsynced tree keeps none.
    pub(super) fn rebase(
        &self,
        motion: Motion<'_>,
        (time, looping): (&mut f32, bool),
        stored: &mut Option<String>,
        leaves: &[LeafTime],
    ) {
        let synced = matches!(motion, Motion::Tree(node) if self.marker_synced(node));
        let now = synced.then(|| self.lead(motion, None, None)).flatten();
        let now = now.filter(|l| !l.markers.is_empty());
        let Some(now) = now else {
            *stored = None;
            return;
        };
        if stored.as_deref() == Some(now.clip.name.as_str()) {
            return;
        }
        let old = stored
            .as_deref()
            .and_then(|l| self.lead(motion, None, Some(l)));
        if let Some(old) = old.filter(|o| stored.as_deref() == Some(o.clip.name.as_str())) {
            // Where the new leader was sampled as a follower, in tree phase.
            let duration = now.clip.duration;
            let was = stored_leaf(leaves, &now.clip.name).filter(|_| duration > 0.0);
            let near = was.map_or(*time, |t| t / duration);
            *time = now.place(old.position(*time, looping), near, looping);
        }
        *stored = Some(now.clip.name.clone());
    }
}

/// The sync markers of the first leaf of `tree` playing `clip`, if any leaf
/// does.
fn leaf_markers<'a>(tree: &'a BlendTree, clip: &str, depth: usize) -> Option<&'a [SyncMarker]> {
    tree.children()
        .find_map(|(motion, _)| match &motion.blend_tree {
            Some(nested) if depth < MAX_TREE_DEPTH => leaf_markers(nested, clip, depth + 1),
            Some(_) => None,
            None => (motion.clip == clip).then_some(&motion.sync_markers[..]),
        })
}

#[cfg(test)]
mod entry_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod fade_tests;
#[cfg(test)]
mod hold_tests;
#[cfg(test)]
mod markers_tests;
#[cfg(test)]
mod repeat_tests;
#[cfg(test)]
mod rig;
#[cfg(test)]
mod root_tests;
#[cfg(test)]
mod tree_tests;
