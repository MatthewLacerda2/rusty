use rusty_asset::mesh_data::SkinData;
use rusty_components::{
    stored_leaf, wrap_delta, Bounds, LeafTime, Motion, Playback, Playhead, RootMotion, Step,
};

use super::super::motion::MotionContext;
use super::RootBone;

/// Taken after `rebase_leaders` (`before_step`), so a change of leader isn't measured as travel.
/// It keeps the outgoing motion, which the step may finish and drop.
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

#[derive(Clone, Debug)]
struct Outgoing {
    tree: bool,
    name: String,
    looping: bool,
    from: f32,
    leaves: Vec<LeafTime>,
}

impl Outgoing {
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
    pub fn before_step(&self, playback: &mut Playback, rooted: bool) -> Option<Before> {
        self.rebase_leaders(playback);
        rooted.then(|| Before::of(playback))
    }

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

/// Unwraps `to` to the lap nearest the travel the tree's `span` makes at the clip's length, so
/// a small re-match back stays a small step back.
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
