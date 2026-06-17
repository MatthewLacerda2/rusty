//! src/asset/gltf_anim.rs — glTF `animation` → [`AnimationClip`] extraction (#80).
//!
//! Reads the document's `animations` into named clips whose tracks are keyed by the
//! *joint slot* of a given skin (the same order #79's [`SkinData`] uses), so the
//! sampler can pose the skeleton directly. A channel that targets a node outside the
//! skin's joints is ignored (it can't drive this mesh's palette). Pure data — the
//! `gltf` crate + `glam`, never wgpu/egui/mlua.

use super::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use glam::{Quat, Vec3};
use std::collections::HashMap;

/// Read every animation in the document into clips bound to `joint_nodes` (the skin's
/// joint slots, by glTF node index). Clips with no track for any of these joints are
/// dropped, so a returned clip always drives at least one of the skin's joints.
pub fn clips_for_skin(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    joint_nodes: &[usize],
) -> Vec<AnimationClip> {
    let slot_of: HashMap<usize, usize> = joint_nodes
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
    slot_of: &HashMap<usize, usize>,
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

/// Read one channel's sampler (times + values) into the matching T/R/S track of a
/// joint. An unreadable or non-T/R/S channel leaves the track untouched.
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
        // `MorphTargetWeights` (and any future output) don't drive joint TRS.
        _ => {}
    }
}

/// glTF `CubicSpline` carries in/out tangents we don't read; treat it as `Linear`,
/// a faithful-enough, deterministic approximation. `Step` and `Linear` map directly.
fn interp_of(i: gltf::animation::Interpolation) -> Interpolation {
    match i {
        gltf::animation::Interpolation::Step => Interpolation::Step,
        _ => Interpolation::Linear,
    }
}

/// Pair vector times+values into a track, guarding the count invariant. A
/// `CubicSpline` output is 3× the key count (in-tangent, value, out-tangent); we
/// keep only the value samples so it degrades cleanly to `Linear`.
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

/// Reconcile an output count with the key count. A 3×-length output is cubic-spline
/// (in, value, out) — keep the middle value of each triple; otherwise truncate to
/// match (a malformed accessor mismatch shouldn't panic the importer).
fn align_values<T: Copy>(key_count: usize, values: Vec<T>) -> Vec<T> {
    if key_count > 0 && values.len() == key_count * 3 {
        return (0..key_count).map(|k| values[k * 3 + 1]).collect();
    }
    let mut values = values;
    values.truncate(key_count);
    values
}
