//! src/app/animation/clip.rs — sampling one clip (#80).
//!
//! The keyframe primitives: a clip's per-joint local TRS at a time (each untouched
//! channel falling back to the bind pose) and which joints a clip keys at all.
//! Pure functions of (skin, clip, time); the palette is built from the bones the
//! pose is written onto (`Scene::build_skin_palettes`).

use crate::asset::anim_data::{AnimationClip, Interpolation, Track};
use crate::asset::mesh_data::{JointTransform, SkinData};
use glam::{Quat, Vec3};

/// Which joint slots `clip` has any keyframes for.
pub(super) fn driven_slots(clip: &AnimationClip, joints: usize) -> Vec<bool> {
    (0..joints)
        .map(|slot| {
            clip.tracks.get(slot).is_some_and(|t| {
                !t.translation.times.is_empty()
                    || !t.rotation.times.is_empty()
                    || !t.scale.times.is_empty()
            })
        })
        .collect()
}

/// Sample each joint's local transform: the clip's track value at `time`, falling
/// back to the joint's bind-pose local for any untouched T/R/S path.
pub(super) fn sampled_locals(
    skin: &SkinData,
    clip: &AnimationClip,
    time: f32,
) -> Vec<JointTransform> {
    skin.local_bind
        .iter()
        .enumerate()
        .map(|(slot, bind)| {
            let Some(track) = clip.tracks.get(slot) else {
                return *bind;
            };
            JointTransform {
                translation: sample_vec(&track.translation, time).unwrap_or(bind.translation),
                rotation: sample_quat(&track.rotation, time).unwrap_or(bind.rotation),
                scale: sample_vec(&track.scale, time).unwrap_or(bind.scale),
            }
        })
        .collect()
}

/// Find the keyframe interval `[i, i+1]` bracketing `time` and the in-segment factor
/// `u` in `[0, 1]`. Clamps to the ends; `None` for an empty track. For a single key
/// the value is held (`(0, 0.0)`).
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
    // `times` is ascending; find the first key strictly past `time`.
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
        // `slerp` is the shortest-arc spherical interpolation glTF mandates for
        // rotations; glam normalizes the result.
        (Interpolation::Linear, Some(&b)) => Some(a.slerp(b, u)),
        _ => Some(a),
    }
}
