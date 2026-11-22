use rusty_asset::anim_data::AnimationClip;
use rusty_asset::animation_graph::LayerBlending;
use rusty_asset::mesh_data::SkinData;
use rusty_components::{
    ActiveMontage, AnimatorComponent, Bounds, MontageEvent, Montages, RootMotion,
    MONTAGE_BLEND_OUT, MONTAGE_END,
};

use super::clip::{driven_slots, sampled_locals};
use super::events::Crossed;
use super::layers::{compose as compose_layer, mask_slots, LayerPose};
use super::motion::Pose;
use super::root_motion::RootBone;

/// Section changes one step may make, at most, so a looping section of almost no length can't
/// spin.
const MAX_SEGMENTS: usize = 16;

pub(super) fn compose(
    pose: &mut Pose,
    skin: &SkinData,
    clips: &[AnimationClip],
    montages: &Montages,
) {
    // `Montages::play` keeps `playing` in slot order, a slot's fading montage before its active
    // one: poses here and root motion in `step` stack in that order.
    for m in &montages.playing {
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
        let reference = match m.slot.blending {
            LayerBlending::Additive => keyed(0.0),
            LayerBlending::Override => Vec::new(),
        };
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
        .map(|info| Crossed {
            name: MONTAGE_BLEND_OUT.to_string(),
            at: 0.0,
            layer: layer_base,
            montage: Some(info),
        })
        .collect();
    for m in &mut montages.playing {
        let layer = layer_base + m.order;
        let event = |name: &str, at: f32, montage: Option<MontageEvent>| Crossed {
            name: name.to_string(),
            at,
            layer,
            montage,
        };
        m.elapsed += dt;
        if let Some(out) = &mut m.out {
            out.elapsed += dt;
        }
        let rate = finite(m.rate * speed).max(0.0);
        let travel = finite(dt * rate);
        let segments = advance(m, travel);
        if m.fires_markers() {
            for (name, at) in crossed(m, &segments, travel) {
                events.push(event(name, at, None));
            }
        }
        let ending = m.out.is_none() && ends_within_blend_out(m, rate);
        if ending && m.blend_out(m.montage.blend_out, false) {
            events.push(event(MONTAGE_BLEND_OUT, 1.0, Some(m.event())));
        }
        if let Some(r) = rooted {
            add_root(m, clips, r, &segments, root);
        }
        if m.out.is_some_and(|o| o.elapsed >= o.duration) {
            events.push(event(MONTAGE_END, 1.0, Some(m.event())));
        }
    }
    montages
        .playing
        .retain(|m| !m.out.is_some_and(|o| o.elapsed >= o.duration));
    events
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    span: (f32, f32),
    offset: f32,
    /// The segment ran onto the end of a section with no next: a marker right on that end fires.
    last: bool,
}

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

fn ends_within_blend_out(m: &ActiveMontage, rate: f32) -> bool {
    if m.next.get(m.section).copied().flatten().is_some() {
        return false;
    }
    let left = m.end(m.section) - m.time;
    left <= 0.0 || (rate > 0.0 && left / rate <= m.montage.blend_out)
}

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
    // Clip seconds (scale 1), `Open`: `advance` already split the travel at each section's end.
    let delta = segments.iter().fold(RootMotion::ZERO, |acc, seg| {
        acc.then(bone.travel(skin, clip, Bounds::Open, 1.0, seg.span))
    });
    let below = root.unwrap_or_default();
    *root = Some(super::stack(below, delta, m.slot.blending, weight));
}

fn find<'c>(clips: &'c [AnimationClip], name: &str) -> Option<&'c AnimationClip> {
    clips.iter().find(|c| c.name == name)
}

fn finite(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(f32::MIN, f32::MAX)
    }
}

#[cfg(test)]
mod marker_tests;
#[cfg(test)]
mod play_tests;
#[cfg(test)]
mod root_tests;
#[cfg(test)]
mod tests;
