//! src/app/animation/root_motion/blend.rs — blending root motion the way poses
//! blend (#1745).
//!
//! A blend tree's delta is its leaves' deltas weighted by the leaves' weights,
//! each leaf measured at the tree's normalized phase (so a walk and a run in
//! step add up to the blended stride). A crossfade lerps the outgoing motion's
//! delta into the incoming one's by the crossfade weight. Layers stack in
//! [`combine`](super::super::step) order: an override layer covering the bone
//! lerps toward its own delta by its weight, an additive one adds its delta
//! scaled by its weight.

use crate::asset::mesh_data::SkinData;
use crate::components::{Bounds, Motion, Playback, Playhead, RootMotion, Step};

use super::super::motion::MotionContext;
use super::RootBone;

/// The motion a crossfade is fading out, captured before the step advances it
/// (the step may finish the crossfade and drop it).
#[derive(Clone, Debug)]
pub struct Outgoing {
    tree: bool,
    name: String,
    looping: bool,
    from: f32,
}

impl Outgoing {
    /// What `playback` is fading out, if it is crossfading.
    pub fn of(playback: &Playback) -> Option<Self> {
        let (tree, name) = match playback.previous_motion()? {
            Motion::Clip(name) => (false, name),
            Motion::Tree(name) => (true, name),
        };
        Some(Self {
            tree,
            name: name.to_string(),
            looping: playback.previous_loop,
            from: playback.previous_time,
        })
    }

    fn motion(&self) -> Motion<'_> {
        match self.tree {
            true => Motion::Tree(&self.name),
            false => Motion::Clip(&self.name),
        }
    }
}

impl MotionContext<'_> {
    /// `playback`'s root motion this step: its current motion over `step`'s
    /// travel, crossfaded from `outgoing` (as it was before the step) by the
    /// crossfade weight the step left. `None` when neither keys the bone.
    pub fn playback_root(
        &self,
        skin: &SkinData,
        bone: &RootBone,
        playback: &Playback,
        step: Step,
        outgoing: Option<&Outgoing>,
    ) -> Option<RootMotion> {
        let current = (playback.motion(), playback.loop_clip, true);
        let current = self.motion_root(skin, bone, current, step.span);
        let Some(out) = outgoing else {
            return current;
        };
        let span = (out.from, playback.previous_time);
        let previous = self.motion_root(skin, bone, (out.motion(), out.looping, false), span);
        if current.is_none() && previous.is_none() {
            return None;
        }
        let (from, to) = (previous.unwrap_or_default(), current.unwrap_or_default());
        Some(from.lerp(to, playback.crossfade_weight()))
    }

    /// One motion's root motion over the playhead travel `span`, sampled the
    /// way [`MotionContext::sample`] samples it: the current clip within its
    /// bounds, an outgoing clip at its raw (end-held) time, a tree's leaves at
    /// its bounded phase. `None` when it doesn't resolve or keys nothing on the
    /// bone's chain.
    fn motion_root(
        &self,
        skin: &SkinData,
        bone: &RootBone,
        (motion, looping, current): (Motion<'_>, bool, bool),
        span: (f32, f32),
    ) -> Option<RootMotion> {
        match motion {
            Motion::Clip(name) => {
                let clip = self.clip(name).filter(|c| bone.keyed_by(c))?;
                let bounds = match current {
                    true => Playhead::clip(clip.duration).bounds(looping),
                    false => Bounds::Open,
                };
                Some(bone.travel(skin, clip, bounds, 1.0, span))
            }
            Motion::Tree(node) => {
                let bounds = Playhead::tree(0.0).bounds(looping);
                let (mut sum, mut total, mut keyed) = (RootMotion::ZERO, 0.0, false);
                self.for_each_leaf_clip(node, |clip, weight| {
                    total += weight;
                    if bone.keyed_by(clip) {
                        keyed = true;
                        let leaf = bone.travel(skin, clip, bounds, clip.duration, span);
                        sum = sum.add_scaled(leaf, weight);
                    }
                });
                (keyed && total > 0.0).then(|| RootMotion::ZERO.add_scaled(sum, 1.0 / total))
            }
        }
    }
}
