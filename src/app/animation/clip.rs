//! src/app/animation/clip.rs — sampling one clip (#80).
//!
//! The keyframe primitives: a clip's per-joint local TRS at a time (each untouched
//! channel falling back to the bind pose), which joints a clip keys at all, and
//! the pure-data palette twin used to check the bone path. Pure functions of
//! (skin, clip, time).

use crate::asset::anim_data::{AnimationClip, Interpolation, Track};
use crate::asset::mesh_data::{JointTransform, SkinData};
use glam::{Mat4, Quat, Vec3};

/// Pose `skin` with `clip` sampled at `time` seconds, returning one mesh-local
/// matrix per joint slot (matching the vertices' `joint_indices`). A joint the clip
/// never animates keeps its bind-pose local transform, so a clip that drives only
/// part of the skeleton leaves the rest at rest. The pure-data twin of the bone
/// path (sample → write bones → `Scene::build_skin_palettes`), with no entities:
/// both yield the same palette for an untouched skeleton.
pub fn sample_palette(skin: &SkinData, clip: &AnimationClip, time: f32) -> Vec<Mat4> {
    let locals = sampled_locals(skin, clip, time);
    let globals = compose_globals(skin, &locals);
    globals
        .iter()
        .zip(skin.inverse_bind.iter())
        .map(|(g, ib)| *g * *ib)
        .collect()
}

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

/// Compose every joint's local transform into a mesh-local global by walking up
/// its parent chain (a root picks up its [`SkinData::root_offset`]). The skin's `parents` form a forest (slots topologically after
/// their parents in glTF), so a single pass left-to-right resolves each global from
/// its already-resolved parent.
pub(super) fn compose_globals(skin: &SkinData, locals: &[JointTransform]) -> Vec<Mat4> {
    let mut globals = vec![Mat4::IDENTITY; locals.len()];
    for (slot, local) in locals.iter().enumerate() {
        let local_mat = local.matrix();
        globals[slot] = match skin.parents.get(slot).copied().flatten() {
            // A forward reference can't happen in well-formed glTF, but guard it:
            // an unresolved parent is treated as a root rather than panicking.
            Some(parent) if parent < slot => globals[parent] * local_mat,
            _ => skin.root_offset(slot) * local_mat,
        };
    }
    globals
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
