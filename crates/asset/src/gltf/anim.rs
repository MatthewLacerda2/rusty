//! glTF `animation` extraction into [`AnimationClip`]s, keyed by the joint slot of a
//! skin (the order of [`SkinData`](crate::SkinData)). A channel targeting a node
//! outside the skin's joints is ignored; one the sampler cannot read is dropped with a
//! warning, as Unity drops a bad curve.

use super::import::File;
use crate::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use glam::{Quat, Vec3};
use rusty_core::collections::Map;

/// Clips with no track for any of these joints are dropped.
pub(super) fn clips_for_skin(file: &File, joint_nodes: &[usize]) -> Vec<AnimationClip> {
    let slot_of: Map<usize, usize> = joint_nodes
        .iter()
        .enumerate()
        .map(|(slot, &node)| (node, slot))
        .collect();

    let mut clips = Vec::new();
    for (i, animation) in file.document.animations().enumerate() {
        let clip = clip_from_animation(&animation, i, file, &slot_of, joint_nodes.len());
        if clip.tracks.iter().any(|t| !t.is_empty()) {
            clips.push(clip);
        }
    }
    clips
}

fn clip_from_animation(
    animation: &gltf::Animation,
    index: usize,
    file: &File,
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
        if let Err(why) = apply_channel(&channel, file, &mut tracks[slot]) {
            let node = channel.target().node();
            let node = node
                .name()
                .map_or_else(|| node.index().to_string(), str::to_string);
            rusty_core::warn_held!(
                "[Asset] {}: animation '{name}', node '{node}': channel dropped: {why}",
                file.name
            );
        }
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
    file: &File,
    track: &mut JointTrack,
) -> Result<(), String> {
    let reader = channel.reader(|b| file.data(b));
    let Some(inputs) = reader.read_inputs() else {
        return Ok(());
    };
    let times: Vec<f32> = inputs.collect();
    if let Some(t) = times.iter().find(|t| !t.is_finite()) {
        return Err(format!("key time {t} is not finite"));
    }
    if let Some(w) = times.windows(2).find(|w| w[0] > w[1]) {
        return Err(format!("key times {} then {} do not ascend", w[0], w[1]));
    }
    let interpolation = interp_of(channel.sampler().interpolation());

    match reader.read_outputs() {
        Some(gltf::animation::util::ReadOutputs::Translations(t)) => {
            track.translation = vec_track(times, t.map(Vec3::from_array).collect(), interpolation)?;
        }
        Some(gltf::animation::util::ReadOutputs::Scales(s)) => {
            track.scale = vec_track(times, s.map(Vec3::from_array).collect(), interpolation)?;
        }
        Some(gltf::animation::util::ReadOutputs::Rotations(r)) => {
            let values = r.into_f32().map(Quat::from_array).collect();
            track.rotation = quat_track(times, values, interpolation)?;
        }
        _ => {}
    }
    Ok(())
}

/// `CubicSpline` carries in/out tangents we don't read; treated as `Linear`.
fn interp_of(i: gltf::animation::Interpolation) -> Interpolation {
    match i {
        gltf::animation::Interpolation::Step => Interpolation::Step,
        _ => Interpolation::Linear,
    }
}

fn vec_track(
    times: Vec<f32>,
    values: Vec<Vec3>,
    interpolation: Interpolation,
) -> Result<Track<Vec3>, String> {
    Ok(Track {
        values: align_values(times.len(), values)?,
        times,
        interpolation,
    })
}

fn quat_track(
    times: Vec<f32>,
    values: Vec<Quat>,
    interpolation: Interpolation,
) -> Result<Track<Quat>, String> {
    Ok(Track {
        values: align_values(times.len(), values)?,
        times,
        interpolation,
    })
}

/// One value per key time, as the sampler indexes them. A `CubicSpline` output is 3x the
/// key count (in-tangent, value, out-tangent); only the values are kept. Any other count
/// is not a curve.
fn align_values<T: Copy>(key_count: usize, values: Vec<T>) -> Result<Vec<T>, String> {
    if values.len() == key_count {
        return Ok(values);
    }
    if values.len() == key_count * 3 {
        return Ok((0..key_count).map(|k| values[k * 3 + 1]).collect());
    }
    Err(format!(
        "{} output values for {key_count} key times",
        values.len()
    ))
}
