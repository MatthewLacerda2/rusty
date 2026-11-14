//! crates/app/src/animation/root_motion/blend.rs — blending root motion the way poses
//! blend (#1745).
//!
//! A blend tree's delta is its leaves' deltas weighted by the leaves' weights,
//! each leaf measured at the tree's normalized phase (so a walk and a run in
//! step add up to the blended stride). A crossfade lerps the outgoing motion's
//! delta into the incoming one's by the crossfade weight. Layers stack in
//! [`combine`](super::super::step) order: an override layer covering the bone
//! lerps toward its own delta by its weight, an additive one adds its delta
//! scaled by its weight.

use rusty_asset::mesh_data::SkinData;
use rusty_components::{
    stored_leaf, wrap_delta, Bounds, LeafTime, Motion, Playback, Playhead, RootMotion, Step,
};

use super::super::motion::MotionContext;
use super::RootBone;

/// Where a playback's motions stand before the step advances them: the
/// current motion's marker-synced leaves (#1896) and the motion a crossfade
/// is fading out (the step may finish the crossfade and drop it). Taken after
/// [`rebase_leaders`](MotionContext::rebase_leaders), so a change of leader
/// isn't measured as travel.
#[derive(Clone, Debug)]
pub struct Before {
    leaves: Vec<LeafTime>,
    outgoing: Option<Outgoing>,
}

impl Before {
    pub fn of(playback: &Playback) -> Self {
        Self {
            leaves: playback.sync.leaves.clone(),
            outgoing: Outgoing::of(playback),
        }
    }
}

/// The motion a crossfade is fading out, as it was before the step.
#[derive(Clone, Debug)]
struct Outgoing {
    tree: bool,
    name: String,
    looping: bool,
    from: f32,
    leaves: Vec<LeafTime>,
}

impl Outgoing {
    /// What `playback` is fading out, if it is crossfading.
    fn of(playback: &Playback) -> Option<Self> {
        let (tree, name) = match playback.previous_motion()? {
            Motion::Clip(name) => (false, name),
            Motion::Tree(name) => (true, name),
        };
        Some(Self {
            tree,
            name: name.to_string(),
            looping: playback.previous_loop,
            from: playback.previous_time,
            leaves: playback.sync.previous_leaves.clone(),
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
    /// Ready `playback` for its step: rebase its trees' leaders (#1896), then,
    /// when root motion is measured (`rooted`), capture where its motions
    /// start, so a change of leader isn't measured as travel.
    pub fn before_step(&self, playback: &mut Playback, rooted: bool) -> Option<Before> {
        self.rebase_leaders(playback);
        rooted.then(|| Before::of(playback))
    }

    /// `playback`'s root motion this step: its current motion over `step`'s
    /// travel, crossfaded from the outgoing one (as `before` the step) by the
    /// crossfade weight the step left. `None` when neither keys the bone.
    pub fn playback_root(
        &self,
        skin: &SkinData,
        bone: &RootBone,
        playback: &Playback,
        step: Step,
        before: &Before,
    ) -> Option<RootMotion> {
        let current = (playback.motion(), playback.loop_clip);
        let leaves = (&before.leaves[..], &playback.sync.leaves[..]);
        let current = self.motion_root(skin, bone, current, step.span, leaves);
        let Some(out) = &before.outgoing else {
            return current;
        };
        let span = (out.from, playback.previous_time);
        let leaves = (&out.leaves[..], &playback.sync.previous_leaves[..]);
        let motion = (out.motion(), out.looping);
        let previous = self.motion_root(skin, bone, motion, span, leaves);
        if current.is_none() && previous.is_none() {
            return None;
        }
        let (from, to) = (previous.unwrap_or_default(), current.unwrap_or_default());
        Some(from.lerp(to, playback.crossfade_weight()))
    }

    /// One motion's root motion over the playhead travel `span` (never
    /// wrapped): a clip within its bounds, a tree's leaves at its bounded phase.
    /// An outgoing clip is measured the same way, so a looping walk fading out
    /// keeps walking across its seam instead of stopping dead there; with root
    /// motion on, the pose's travel is stripped anyway, so the end-held pose an
    /// outgoing clip samples doesn't show. A marker-synced tree's leaves
    /// travel between where `leaves` (before, after) put them. `None` when it
    /// doesn't resolve or keys nothing on the bone's chain.
    fn motion_root(
        &self,
        skin: &SkinData,
        bone: &RootBone,
        (motion, looping): (Motion<'_>, bool),
        span: (f32, f32),
        (before, after): (&[LeafTime], &[LeafTime]),
    ) -> Option<RootMotion> {
        match motion {
            Motion::Clip(name) => {
                let clip = self.clip(name).filter(|c| bone.keyed_by(c))?;
                let bounds = Playhead::clip(clip.duration).bounds(looping);
                Some(bone.travel(skin, clip, bounds, 1.0, span))
            }
            Motion::Tree(node) => {
                let bounds = Playhead::tree(0.0).bounds(looping);
                let at = |phase: f32| bounds.apply(phase);
                let sync = (span.0 != span.1)
                    .then(|| {
                        self.tree_sync(node, at(span.0), looping)
                            .zip(self.tree_sync(node, at(span.1), looping))
                    })
                    .flatten();
                let (mut sum, mut total, mut keyed) = (RootMotion::ZERO, 0.0, false);
                self.for_each_leaf_clip(node, |clip, weight, markers| {
                    total += weight;
                    if !bone.keyed_by(clip) {
                        return;
                    }
                    keyed = true;
                    let leaf = match &sync {
                        // A leaf of a marker-synced tree (#1896) travels from
                        // where it was to where it is, the occurrence of its
                        // markers nearest where it was, never jumping one.
                        Some((a, b)) => {
                            let name = &clip.name;
                            let from = stored_leaf(before, name);
                            let from = from
                                .unwrap_or_else(|| a.leaf_time(clip, markers, at(span.0), None));
                            let to = stored_leaf(after, name);
                            let to = to.unwrap_or_else(|| {
                                b.leaf_time(clip, markers, at(span.1), Some(from))
                            });
                            let (bounds, to) = leaf_travel(bounds, clip.duration, from, to, span);
                            bone.travel(skin, clip, bounds, 1.0, (from, to))
                        }
                        None => bone.travel(skin, clip, bounds, clip.duration, span),
                    };
                    sum = sum.add_scaled(leaf, weight);
                });
                (keyed && total > 0.0).then(|| RootMotion::ZERO.add_scaled(sum, 1.0 / total))
            }
        }
    }
}

/// A marker-synced leaf's travel in its own seconds, from `from` to `to`
/// (both within the clip): the tree's `bounds` scaled to the clip, and `to`
/// unwrapped round a loop's seam to the lap nearest the travel the tree's
/// `span` makes at the clip's length, so a small re-match back is a small
/// step back, not most of a lap forward.
fn leaf_travel(
    bounds: Bounds,
    duration: f32,
    from: f32,
    to: f32,
    span: (f32, f32),
) -> (Bounds, f32) {
    match bounds {
        Bounds::Wrap(_) if duration > 0.0 => {
            let expected = (span.1 - span.0) * duration;
            let to = from + expected + wrap_delta(to - from - expected, duration);
            (Bounds::Wrap(duration), to)
        }
        Bounds::Clamp(_) => (Bounds::Clamp(duration), to),
        _ => (Bounds::Open, to),
    }
}
