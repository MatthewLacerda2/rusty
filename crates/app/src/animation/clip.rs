use glam::{Quat, Vec3};
use rusty_asset::anim_data::{AnimationClip, Interpolation, Track};
use rusty_asset::mesh_data::{JointTransform, SkinData};

pub(super) fn driven_slots(clip: &AnimationClip, joints: usize) -> Vec<bool> {
    let mut driven = vec![false; joints];
    mark_driven(clip, &mut driven);
    driven
}

pub(super) fn mark_driven(clip: &AnimationClip, driven: &mut [bool]) {
    for (slot, d) in driven.iter_mut().enumerate() {
        *d |= clip.tracks.get(slot).is_some_and(|t| {
            !t.translation.times.is_empty()
                || !t.rotation.times.is_empty()
                || !t.scale.times.is_empty()
        });
    }
}

pub(super) fn sampled_locals(
    skin: &SkinData,
    clip: &AnimationClip,
    time: f32,
) -> Vec<JointTransform> {
    let mut out = Vec::with_capacity(skin.local_bind.len());
    sample_into(skin, clip, time, &mut out);
    out
}

pub(super) fn sample_into(
    skin: &SkinData,
    clip: &AnimationClip,
    time: f32,
    out: &mut Vec<JointTransform>,
) {
    out.clear();
    out.extend((0..skin.local_bind.len()).map(|slot| sample_joint(skin, clip, slot, time)));
}

pub(super) fn sample_joint(
    skin: &SkinData,
    clip: &AnimationClip,
    slot: usize,
    time: f32,
) -> JointTransform {
    let bind = skin.local_bind.get(slot).copied().unwrap_or_default();
    let Some(track) = clip.tracks.get(slot) else {
        return bind;
    };
    JointTransform {
        translation: sample_vec(&track.translation, time).unwrap_or(bind.translation),
        rotation: sample_quat(&track.rotation, time).unwrap_or(bind.rotation),
        scale: sample_vec(&track.scale, time).unwrap_or(bind.scale),
    }
}

fn locate(times: &[f32], time: f32) -> Option<(usize, f32)> {
    if times.is_empty() {
        return None;
    }
    if time <= times[0] || times.len() == 1 {
        return Some((0, 0.0));
    }
    if time >= times[times.len() - 1] {
        return Some((times.len() - 1, 0.0));
    }
    let next = times
        .iter()
        .position(|&t| t > time)
        .unwrap_or(times.len() - 1);
    let i = next - 1;
    let span = times[next] - times[i];
    let u = if span > 0.0 {
        (time - times[i]) / span
    } else {
        0.0
    };
    Some((i, u))
}

fn sample_vec(track: &Track<Vec3>, time: f32) -> Option<Vec3> {
    let (i, u) = locate(&track.times, time)?;
    let a = track.values[i];
    match (track.interpolation, track.values.get(i + 1)) {
        (Interpolation::Linear, Some(&b)) => Some(a.lerp(b, u)),
        _ => Some(a),
    }
}

fn sample_quat(track: &Track<Quat>, time: f32) -> Option<Quat> {
    let (i, u) = locate(&track.times, time)?;
    let a = track.values[i];
    match (track.interpolation, track.values.get(i + 1)) {
        (Interpolation::Linear, Some(&b)) => Some(a.slerp(b, u)),
        _ => Some(a),
    }
}
