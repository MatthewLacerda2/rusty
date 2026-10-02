//! src/app/animation/motion.rs — sampling and timing one layer's playback (#457).
//!
//! A [`Playback`] plays a motion — one clip, or a blend tree of clips — and,
//! during a crossfade, fades out another. This turns that into a [`Pose`]: per
//! joint slot the local TRS, or `None` where nothing playing keys the joint. A
//! blend tree samples every weighted child at the **same normalized phase**
//! (`phase × child length`), so cycles of different lengths stay in step, and
//! blends them in TRS space like a crossfade does.

use crate::asset::anim_data::AnimationClip;
use crate::asset::animation_graph::{GraphNode, StateMachine};
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{AnimatorParameters, Motion, Playback, Playhead};

use super::blend_poses;
use super::blend_tree::tree_weights;
use super::clip::{driven_slots, sampled_locals};
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
        let time = |t: f32| if at_start { 0.0 } else { t };
        let mut pose = self.sample_motion(skin, playback.motion(), time(playback.time))?;
        let outgoing = playback
            .previous_motion()
            .and_then(|m| self.sample_motion(skin, m, time(playback.previous_time)));
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

    fn sample_motion(&self, skin: &SkinData, motion: Motion<'_>, time: f32) -> Option<Sampled> {
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
                let node = self.tree_node(node)?;
                let phase = if node.is_loop {
                    time.rem_euclid(1.0)
                } else {
                    time.clamp(0.0, 1.0)
                };
                let mut blended: Option<Sampled> = None;
                let mut total = 0.0;
                for (clip, weight) in self.weighted_children(node) {
                    let child = Sampled {
                        locals: sampled_locals(skin, clip, phase * clip.duration),
                        driven: driven_slots(clip, joints),
                    };
                    total += weight;
                    blended = Some(match blended {
                        None => child,
                        Some(mut acc) => {
                            acc.locals = blend_poses(&acc.locals, &child.locals, weight / total);
                            for (d, c) in acc.driven.iter_mut().zip(child.driven) {
                                *d |= c;
                            }
                            acc
                        }
                    });
                }
                blended
            }
        }
    }

    /// A blend-tree node's resolvable children with a positive weight.
    fn weighted_children(&self, node: &GraphNode) -> Vec<(&AnimationClip, f32)> {
        let Some(tree) = &node.blend_tree else {
            return Vec::new();
        };
        tree.clips()
            .into_iter()
            .zip(tree_weights(tree, self.parameters))
            .filter(|&(_, w)| w > 0.0)
            .filter_map(|(name, w)| Some((self.clip(name)?, w)))
            .collect()
    }

    /// How `motion`'s playhead moves: a clip in seconds wrapping at its duration
    /// (0, never wrap, when it doesn't resolve); a tree normalized, at one cycle
    /// per weighted-mean child length (Unity's blended duration), standing still
    /// when it has no length.
    pub fn playhead(&self, motion: Motion<'_>) -> Playhead {
        match motion {
            Motion::Clip(name) => Playhead::clip(self.clip(name).map_or(0.0, |c| c.duration)),
            Motion::Tree(node) => {
                let children = self.tree_node(node).map(|n| self.weighted_children(n));
                let (sum, total) = children
                    .unwrap_or_default()
                    .iter()
                    .fold((0.0, 0.0), |(sum, total), (clip, w)| {
                        (sum + clip.duration * w, total + w)
                    });
                let length = if total > 0.0 { sum / total } else { 0.0 };
                Playhead {
                    per_second: if length > 0.0 { 1.0 / length } else { 0.0 },
                    wrap: 1.0,
                }
            }
        }
    }

    /// Advance `playback` by `dt` at `speed`, each playhead at its motion's rate,
    /// and return the animation events (#459) the current motion crossed, tagged
    /// with `layer`.
    pub fn advance(
        &self,
        playback: &mut Playback,
        dt: f32,
        speed: f32,
        layer: usize,
    ) -> Vec<Crossed> {
        let current = self.playhead(playback.motion());
        let previous = playback
            .previous_motion()
            .map_or(1.0, |m| self.playhead(m).per_second);
        let span = playback.advance(dt, speed, current, previous);
        self.crossed(playback, current, span, layer)
    }
}
