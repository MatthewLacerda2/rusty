//! src/app/animation/montage/ — playing montages over the layers (#1894).
//!
//! Each step, every montage an animator plays ([`Montages`]):
//! - blends in over its `blend_in` and out over its blend out, on unscaled
//!   time like a crossfade;
//! - advances its playhead at its rate × the animator's speed (never
//!   backwards: a reversed or stopped animator holds it), through its
//!   sections: a section that ends goes on to its `next` (itself loops it,
//!   carrying the leftover time), and one with none holds at its end;
//! - starts blending out when it is in a section with no `next` and that
//!   section's end is no further away than its blend out (Unreal's default
//!   blend-out trigger), so it is gone as the clip ends;
//! - fires its markers crossed this step, with the same half-open rule as a
//!   node's events, plus [`MONTAGE_BLEND_OUT`] and [`MONTAGE_END`] once each;
//! - adds its root motion over the travel when its slot's mask covers the root
//!   bone, stacked like a layer's (`super::stack`).
//!
//! Its pose composes above every layer, slot by slot in the slot stack's order
//! (a slot's blending-out montage under its active one), exactly as a layer's
//! does (`layers::compose`). Pure functions of (skin, clips, montages, dt).

use crate::asset::anim_data::AnimationClip;
use crate::asset::mesh_data::SkinData;
use crate::components::{
    ActiveMontage, AnimatorComponent, Bounds, Montages, RootMotion, MONTAGE_BLEND_OUT, MONTAGE_END,
};

use super::clip::{driven_slots, sampled_locals};
use super::events::Crossed;
use super::layers::{compose as compose_layer, mask_slots, LayerPose};
use super::motion::Pose;
use super::root_motion::RootBone;

/// At most this many section changes in one step, so a looping section of
/// almost no length can't spin.
const MAX_SEGMENTS: usize = 16;

/// The montages' poses, composed onto `pose` above every layer.
pub(super) fn compose(
    pose: &mut Pose,
    skin: &SkinData,
    clips: &[AnimationClip],
    montages: &Montages,
) {
    for m in in_slot_order(montages) {
        let weight = m.weight();
        let Some(clip) = find(clips, &m.montage.clip).filter(|_| weight > 0.0) else {
            continue;
        };
        let joints = skin.local_bind.len();
        let keyed = |time: f32| -> Pose {
            let driven = driven_slots(clip, joints);
            let locals = sampled_locals(skin, clip, time);
            locals
                .into_iter()
                .zip(driven)
                .map(|(l, d)| d.then_some(l))
                .collect()
        };
        let top = keyed(m.time);
        let reference = keyed(0.0);
        let mask = mask_slots(skin, &m.slot.mask);
        let layer = LayerPose {
            blending: m.slot.blending,
            weight,
            mask: &mask,
            pose: &top,
            reference: &reference,
        };
        compose_layer(pose, &layer, &skin.local_bind);
    }
}

/// Advance every montage of `anim` by `dt` at its speed, returning the
/// events crossed; each montage's layer index is `layer_base` + its slot's
/// place. With the skin and its root bone (`rooted`), their root motion
/// stacks onto `root`.
pub(super) fn step(
    anim: &mut AnimatorComponent,
    clips: &[AnimationClip],
    dt: f32,
    layer_base: usize,
    rooted: Option<(&SkinData, &RootBone)>,
    root: &mut Option<RootMotion>,
) -> Vec<Crossed> {
    let (montages, speed) = (&mut anim.montages, anim.speed);
    let mut events: Vec<Crossed> = std::mem::take(&mut montages.pending)
        .into_iter()
        .map(|name| Crossed {
            name,
            at: 0.0,
            layer: layer_base,
        })
        .collect();
    for m in &mut montages.playing {
        let layer = layer_base + m.order;
        let event = |name: &str, at: f32| Crossed {
            name: name.to_string(),
            at,
            layer,
        };
        m.elapsed += dt;
        if let Some(out) = &mut m.out {
            out.elapsed += dt;
        }
        let rate = finite(m.rate * speed).max(0.0);
        let travel = finite(dt * rate);
        let segments = advance(m, travel);
        for (name, at) in crossed(m, &segments, travel) {
            events.push(event(name, at));
        }
        if m.out.is_none() && ends_within_blend_out(m, rate) && m.blend_out(m.montage.blend_out) {
            events.push(event(MONTAGE_BLEND_OUT, 1.0));
        }
        if let Some(r) = rooted {
            add_root(m, clips, r, &segments, root);
        }
        if m.out.is_some_and(|o| o.elapsed >= o.duration) {
            events.push(event(MONTAGE_END, 1.0));
        }
    }
    montages
        .playing
        .retain(|m| !m.out.is_some_and(|o| o.elapsed >= o.duration));
    events
}

/// One stretch of travel within a section: `(from, to)` in clip seconds, and
/// how far into the step's travel it began.
#[derive(Clone, Copy, Debug)]
struct Segment {
    span: (f32, f32),
    offset: f32,
    /// It ran onto the end of a section with no next: the montage's last frame.
    last: bool,
}

/// Move `m`'s playhead `travel` clip seconds through its sections.
fn advance(m: &mut ActiveMontage, travel: f32) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut left = travel;
    for _ in 0..MAX_SEGMENTS {
        let end = m.end(m.section);
        let from = m.time.min(end);
        let offset = travel - left;
        if from + left < end {
            m.time = from + left;
            out.push(Segment {
                span: (from, m.time),
                offset,
                last: false,
            });
            break;
        }
        let next = m.next.get(m.section).copied().flatten();
        out.push(Segment {
            span: (from, end),
            offset,
            last: next.is_none(),
        });
        left -= end - from;
        m.time = end;
        let Some(next) = next else { break };
        m.section = next;
        m.time = m.start(next);
        if left <= 0.0 && end > from {
            break;
        }
    }
    out
}

/// The markers `m` crossed over `segments`, each with when in the step: a
/// marker in `[from, to)`, or on the very end the montage ran onto.
fn crossed<'m>(m: &'m ActiveMontage, segments: &[Segment], travel: f32) -> Vec<(&'m str, f32)> {
    let mut out = Vec::new();
    for seg in segments {
        let (from, to) = seg.span;
        if to <= from {
            continue;
        }
        for event in &m.montage.events {
            let mark = event.time.clamp(0.0, m.length.max(0.0));
            if from <= mark && (mark < to || (seg.last && mark == to)) {
                let at = match travel > 0.0 {
                    true => ((seg.offset + mark - from) / travel).clamp(0.0, 1.0),
                    false => 0.0,
                };
                out.push((event.name.as_str(), at));
            }
        }
    }
    out
}

/// `m` is in a section with no next and reaches its end within its blend out
/// at `rate` (or is already there).
fn ends_within_blend_out(m: &ActiveMontage, rate: f32) -> bool {
    if m.next.get(m.section).copied().flatten().is_some() {
        return false;
    }
    let left = m.end(m.section) - m.time;
    left <= 0.0 || (rate > 0.0 && left / rate <= m.montage.blend_out)
}

/// Stack `m`'s root motion over `segments` onto `root`, where its slot covers
/// the bone and its clip keys it.
fn add_root(
    m: &ActiveMontage,
    clips: &[AnimationClip],
    (skin, bone): (&SkinData, &RootBone),
    segments: &[Segment],
    root: &mut Option<RootMotion>,
) {
    let weight = m.weight();
    let covers = mask_slots(skin, &m.slot.mask).get(bone.slot()) == Some(&true);
    let Some(clip) = find(clips, &m.montage.clip) else {
        return;
    };
    if weight <= 0.0 || !covers || !bone.keyed_by(clip) {
        return;
    }
    let delta = segments.iter().fold(RootMotion::ZERO, |acc, seg| {
        acc.then(bone.travel(skin, clip, Bounds::Open, 1.0, seg.span))
    });
    let below = root.unwrap_or_default();
    *root = Some(super::stack(below, delta, m.slot.blending, weight));
}

/// The montages in slot-stack order, a slot's earlier starts first.
fn in_slot_order(montages: &Montages) -> Vec<&ActiveMontage> {
    let mut ordered: Vec<&ActiveMontage> = montages.playing.iter().collect();
    ordered.sort_by_key(|m| m.order);
    ordered
}

fn find<'c>(clips: &'c [AnimationClip], name: &str) -> Option<&'c AnimationClip> {
    clips.iter().find(|c| c.name == name)
}

/// `x` as a finite number, NaN as 0.
fn finite(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(f32::MIN, f32::MAX)
    }
}

#[cfg(test)]
mod play_tests;
#[cfg(test)]
mod root_tests;
#[cfg(test)]
mod tests;
