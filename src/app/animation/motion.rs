//! src/app/animation/motion.rs — sampling and timing one layer's playback (#457).
//!
//! A [`Playback`] plays a motion — one clip, or a blend tree of clips and
//! nested trees (#1746) — and,
//! during a crossfade, fades out another. This turns that into a [`Pose`]: per
//! joint slot the local TRS, or `None` where nothing playing keys the joint. A
//! blend tree samples every weighted leaf clip at the **same normalized phase**
//! (`phase × clip length`), so cycles of different lengths stay in step, and
//! blends them in TRS space like a crossfade does, through one reusable buffer
//! rather than a pose per leaf. A tree whose every leaf carries sync markers
//! samples them marker to marker instead (#1896, `sync`).

use crate::asset::anim_data::AnimationClip;
use crate::asset::animation_graph::{GraphNode, StateMachine, SyncMarker};
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{
    stored_leaf, AnimatorParameters, LeafTime, Motion, Playback, Playhead, Step,
};

use super::blend_joint;
use super::blend_poses;
use super::blend_tree::for_each_leaf;
use super::clip::{driven_slots, mark_driven, sample_into, sampled_locals};
use super::events::Crossed;

/// Per joint slot, the local TRS a layer gives it, or `None` where it is not
/// driven.
pub type Pose = Vec<Option<JointTransform>>;

/// What a motion resolves against: the mesh's clips, the layer's state machine
/// (for blend-tree nodes) and the animator's parameters (for their weights).
pub struct MotionContext<'a> {
    pub clips: &'a [AnimationClip],
    pub machine: Option<&'a StateMachine>,
    pub parameters: &'a AnimatorParameters,
}

/// A sampled motion: every joint's local TRS (bind pose where unkeyed) and which
/// joints it keys.
struct Sampled {
    locals: Vec<JointTransform>,
    driven: Vec<bool>,
}

impl MotionContext<'_> {
    pub(super) fn clip(&self, name: &str) -> Option<&AnimationClip> {
        self.clips.iter().find(|c| c.name == name)
    }

    /// The blend-tree node `name` in this layer's machine.
    fn tree_node(&self, name: &str) -> Option<&GraphNode> {
        self.machine?.node(name).filter(|n| n.blend_tree.is_some())
    }

    /// `playback`'s pose: the current motion, crossfaded from the outgoing one.
    /// With `at_start`, both motions are sampled at their first frame instead —
    /// an additive layer's reference pose. `None` when the current motion does not
    /// resolve.
    pub fn sample(&self, skin: &SkinData, playback: &Playback, at_start: bool) -> Option<Pose> {
        // An additive reference samples first frames: no leaf keeps a place.
        let time = |t: f32, leaves| {
            if at_start {
                (0.0, &[][..])
            } else {
                (t, leaves)
            }
        };
        let current = (playback.motion(), playback.loop_clip);
        let at = time(playback.time, &playback.sync.leaves[..]);
        let mut pose = self.sample_motion(skin, current, at)?;
        let outgoing = playback.previous_motion().and_then(|m| {
            let motion = (m, playback.previous_loop);
            let at = time(playback.previous_time, &playback.sync.previous_leaves[..]);
            self.sample_motion(skin, motion, at)
        });
        if let Some(from) = outgoing {
            pose.locals = blend_poses(&from.locals, &pose.locals, playback.crossfade_weight());
            for (d, p) in pose.driven.iter_mut().zip(from.driven) {
                *d |= p;
            }
        }
        let Sampled { locals, driven } = pose;
        Some(
            locals
                .into_iter()
                .zip(driven)
                .map(|(l, d)| d.then_some(l))
                .collect(),
        )
    }

    /// One motion's pose at `time`, under its `loop_clip` flag: a tree's phase is
    /// bounded the way its playhead advances ([`Playhead::bounds`]), so the flag,
    /// not the graph node's `is_loop`, decides both (#1362). A marker-synced
    /// tree's followers are placed nearest where `leaves` last put them.
    fn sample_motion(
        &self,
        skin: &SkinData,
        (motion, looping): (Motion<'_>, bool),
        (time, leaves): (f32, &[LeafTime]),
    ) -> Option<Sampled> {
        let joints = skin.local_bind.len();
        match motion {
            Motion::Clip(name) => {
                let clip = self.clip(name)?;
                Some(Sampled {
                    locals: sampled_locals(skin, clip, time),
                    driven: driven_slots(clip, joints),
                })
            }
            Motion::Tree(node) => {
                let phase = Playhead::tree(0.0).bounds(looping).apply(time);
                let sync = self.tree_sync(node, phase, looping);
                let mut blended: Option<Sampled> = None;
                let mut scratch = Vec::new();
                let mut total = 0.0;
                self.for_each_leaf_clip(node, |clip, weight, markers| {
                    total += weight;
                    let at = sync.as_ref().map_or(phase * clip.duration, |s| {
                        s.leaf_time(clip, markers, phase, stored_leaf(leaves, &clip.name))
                    });
                    sample_into(skin, clip, at, &mut scratch);
                    let Some(acc) = &mut blended else {
                        // The first leaf is the pose as sampled; its buffer
                        // becomes the output, and later leaves reuse a fresh one.
                        let mut driven = vec![false; joints];
                        mark_driven(clip, &mut driven);
                        let locals = std::mem::take(&mut scratch);
                        blended = Some(Sampled { locals, driven });
                        return;
                    };
                    let t = weight / total;
                    for (a, &b) in acc.locals.iter_mut().zip(&scratch) {
                        *a = blend_joint(*a, b, t);
                    }
                    mark_driven(clip, &mut acc.driven);
                });
                blended
            }
        }
    }

    /// Call `visit` with every resolvable leaf clip of the blend-tree node
    /// `node` that has a positive weight, depth first in authored order, with
    /// its weight and sync markers (#1896).
    pub(super) fn for_each_leaf_clip(
        &self,
        node: &str,
        mut visit: impl FnMut(&AnimationClip, f32, &[SyncMarker]),
    ) {
        let Some(tree) = self.tree_node(node).and_then(|n| n.blend_tree.as_ref()) else {
            return;
        };
        for_each_leaf(tree, self.parameters, &mut |leaf| {
            if let Some(clip) = self.clip(leaf.clip) {
                visit(clip, leaf.weight, leaf.sync_markers);
            }
        });
    }

    /// How `motion`'s playhead moves: a clip in seconds wrapping at its duration
    /// ([`Playhead::unresolved`], never wrap, when it doesn't resolve); a tree
    /// normalized, at one cycle per weighted-mean child length (Unity's blended
    /// duration), standing still when it has no length.
    pub fn playhead(&self, motion: Motion<'_>) -> Playhead {
        match motion {
            Motion::Clip(name) => self
                .clip(name)
                .map_or(Playhead::unresolved(), |c| Playhead::clip(c.duration)),
            Motion::Tree(node) => {
                let (mut sum, mut total) = (0.0, 0.0);
                self.for_each_leaf_clip(node, |clip, w, _| {
                    sum += clip.duration * w;
                    total += w;
                });
                let length = if total > 0.0 { sum / total } else { 0.0 };
                Playhead::tree(if length > 0.0 { 1.0 / length } else { 0.0 })
            }
        }
    }

    /// Advance `playback` by `dt` at `speed`, each playhead at its motion's rate,
    /// and return the animation events (#459) the current motion crossed, tagged
    /// with `layer`, and the current playhead's [`Step`] (what root motion is
    /// measured over, #1745). A synced crossfade's lighter motion follows the
    /// heavier one, and a marker-synced tree's leaves are tracked where they
    /// are sampled (#1896, `sync`). Call [`rebase_leaders`](Self::rebase_leaders)
    /// first.
    pub fn advance(
        &self,
        playback: &mut Playback,
        dt: f32,
        speed: f32,
        layer: usize,
    ) -> (Vec<Crossed>, Step) {
        let current = self.playhead(playback.motion());
        let previous = playback
            .previous_motion()
            .map_or(1.0, |m| self.playhead(m).per_second);
        let step = if playback.sync.following && playback.is_crossfading() {
            self.advance_synced(playback, (dt, speed), current, previous)
        } else {
            let forward = playback.scaled(dt, speed) >= 0.0;
            let step = playback.advance(dt, speed, current, previous);
            playback.repay(step, forward)
        };
        let mut leaves = std::mem::take(&mut playback.sync.leaves);
        let at = (playback.time, playback.loop_clip);
        self.track_leaves(Some(playback.motion()), at, &mut leaves);
        playback.sync.leaves = leaves;
        let mut leaves = std::mem::take(&mut playback.sync.previous_leaves);
        let at = (playback.previous_time, playback.previous_loop);
        self.track_leaves(playback.previous_motion(), at, &mut leaves);
        playback.sync.previous_leaves = leaves;
        playback.states.travel = step.span.1 - step.span.0; // a state's time (#1901)
        (self.crossed(playback, current, step, layer), step)
    }

    /// [`rebase`](Self::rebase) the current tree and, while any crossfade (synced
    /// or not) fades it out, the previous one: a change of leader carries a
    /// marker-synced tree's phase over (#1896), so neither the pose nor the
    /// root motion read from its leaves jumps. Before the step, and before root
    /// motion reads where the motions start from.
    pub(super) fn rebase_leaders(&self, playback: &mut Playback) {
        let Playback {
            current_clip,
            current_tree,
            time,
            loop_clip,
            previous_tree,
            previous_time,
            previous_loop,
            sync,
            ..
        } = playback;
        let motion = match current_tree {
            Some(node) => Motion::Tree(node),
            None => Motion::Clip(current_clip),
        };
        self.rebase(motion, (time, *loop_clip), &mut sync.leader, &sync.leaves);
        if let Some(node) = previous_tree {
            let at = (previous_time, *previous_loop);
            let (leader, leaves) = (&mut sync.previous_leader, &sync.previous_leaves);
            self.rebase(Motion::Tree(node), at, leader, leaves);
        }
    }
}
