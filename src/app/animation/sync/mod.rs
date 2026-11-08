//! src/app/animation/sync/ — keeping motions in step: sync groups and sync
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
//!   its phase across a change of dominant leaf ([`MotionContext::rebase`]).
//!
//! Pure functions of (graph, clips, parameters, playheads): deterministic.

use crate::asset::anim_data::AnimationClip;
use crate::asset::animation_graph::{AnimationGraph, BlendTree, SyncMarker, MAX_TREE_DEPTH};
use crate::components::{AnimatorComponent, Motion, Playback, Playhead};

use super::blend_tree::dominant_leaf;
use super::motion::MotionContext;
pub use markers::SyncPos;
use markers::{place, position};

mod fade;
mod markers;

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
    /// leader's place between its markers.
    pub fn leaf_time(&self, clip: &AnimationClip, markers: &[SyncMarker], phase: f32) -> f32 {
        let natural = phase * clip.duration;
        if clip.name == self.leader {
            return natural;
        }
        place(markers, clip.duration, self.pos, natural, self.looping)
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
        time: &mut f32,
        looping: bool,
        stored: &mut Option<String>,
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
            *time = now.place(old.position(*time, looping), *time, looping);
        }
        *stored = Some(now.clip.name.clone());
    }

    /// Place the incoming motion at the phase of the one a node of its group
    /// was entered from, if that is still to do. Nothing travels, so nothing
    /// fires.
    pub(super) fn place_pending(&self, playback: &mut Playback) {
        let Some(from) = playback.sync.from.take() else {
            return;
        };
        let out = self.lead(from.motion(), from.node.as_deref(), from.leader.as_deref());
        let node = playback.current_node.as_deref();
        let Some(lead) = self.lead(playback.motion(), node, None) else {
            return;
        };
        if let Some(out) = out {
            let pos = out.position(from.time, from.looping);
            playback.time = lead.place(pos, playback.time, playback.loop_clip);
        }
        if lead.tree && !lead.markers.is_empty() {
            playback.sync.leader = Some(lead.clip.name.clone());
        }
    }
}

/// Make every layer's pending sync-group placement (#1896): paused or not, a
/// node entered from one of its group starts at the outgoing phase.
pub(super) fn place_pending(
    anim: &mut AnimatorComponent,
    clips: &[AnimationClip],
    graph: Option<&AnimationGraph>,
) {
    let AnimatorComponent {
        base,
        layers,
        parameters,
        ..
    } = anim;
    let playbacks = std::iter::once(base).chain(layers.iter_mut().map(|l| &mut l.playback));
    for (layer, playback) in playbacks.enumerate() {
        let context = MotionContext {
            clips,
            machine: graph.and_then(|g| g.machine(layer)),
            parameters,
        };
        context.place_pending(playback);
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
mod events_tests;
#[cfg(test)]
mod fade_tests;
#[cfg(test)]
mod markers_tests;
#[cfg(test)]
mod rig;
#[cfg(test)]
mod tree_tests;
