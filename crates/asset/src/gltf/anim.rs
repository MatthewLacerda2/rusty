//! glTF `animation` extraction into [`AnimationClip`]s, keyed by the joint slot of a
//! skin (the order of [`SkinData`](crate::SkinData)). A channel targeting a node
//! outside the skin's joints is ignored.

use crate::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use glam::{Quat, Vec3};
use rusty_core::collections::Map;

/// Clips with no track for any of these joints are dropped.
pub fn clips_for_skin(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    joint_nodes: &[usize],
) -> Vec<AnimationClip> {
    let slot_of: Map<usize, usize> = joint_nodes
        .iter()
        .enumerate()
        .map(|(slot, &node)| (node, slot))
        .collect();

    let mut clips = Vec::new();
    for (i, animation) in document.animations().enumerate() {
        let clip = clip_from_animation(&animation, i, buffers, &slot_of, joint_nodes.len());
        if clip.tracks.iter().any(|t| !t.is_empty()) {
            clips.push(clip);
        }
    }
    clips
}

fn clip_from_animation(
    animation: &gltf::Animation,
    index: usize,
    buffers: &[gltf::buffer::Data],
    slot_of: &Map<usize, usize>,
    joint_count: usize,
) -> AnimationClip {
    let name = animation
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("clip_{index}"));
    let mut tracks = vec![JointTrack::default(); joint_count];

    for channel in animation.channels() {
        let Some(&slot) = slot_of.get(&channel.target().node().index()) else {
            continue;
        };
        apply_channel(&channel, buffers, &mut tracks[slot]);
    }

    let mut clip = AnimationClip {
        name,
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

fn apply_channel(
    channel: &gltf::animation::Channel,
    buffers: &[gltf::buffer::Data],
    track: &mut JointTrack,
) {
    let reader = channel.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));
    let Some(inputs) = reader.read_inputs() else {
        return;
    };
    let times: Vec<f32> = inputs.collect();
    let interpolation = interp_of(channel.sampler().interpolation());

    match reader.read_outputs() {
        Some(gltf::animation::util::ReadOutputs::Translations(t)) => {
            track.translation = vec_track(times, t.map(Vec3::from_array).collect(), interpolation);
        }
        Some(gltf::animation::util::ReadOutputs::Scales(s)) => {
            track.scale = vec_track(times, s.map(Vec3::from_array).collect(), interpolation);
        }
        Some(gltf::animation::util::ReadOutputs::Rotations(r)) => {
            let values = r.into_f32().map(Quat::from_array).collect();
            track.rotation = quat_track(times, values, interpolation);
        }
        _ => {}
    }
}

/// `CubicSpline` carries in/out tangents we don't read; treated as `Linear`.
fn interp_of(i: gltf::animation::Interpolation) -> Interpolation {
    match i {
        gltf::animation::Interpolation::Step => Interpolation::Step,
        _ => Interpolation::Linear,
    }
}

/// A `CubicSpline` output is 3x the key count (in-tangent, value, out-tangent); only the
/// values are kept.
fn vec_track(times: Vec<f32>, mut values: Vec<Vec3>, interpolation: Interpolation) -> Track<Vec3> {
    values = align_values(times.len(), values);
    Track {
        times,
        values,
        interpolation,
    }
}

fn quat_track(times: Vec<f32>, mut values: Vec<Quat>, interpolation: Interpolation) -> Track<Quat> {
    values = align_values(times.len(), values);
    Track {
        times,
        values,
        interpolation,
    }
}

fn align_values<T: Copy>(key_count: usize, values: Vec<T>) -> Vec<T> {
    if key_count > 0 && values.len() == key_count * 3 {
        return (0..key_count).map(|k| values[k * 3 + 1]).collect();
    }
    let mut values = values;
    values.truncate(key_count);
    values
}
