//! src/app/animation.rs — the deterministic keyframe sampler (#80).
//!
//! Poses an imported skeleton ([`SkinData`]) from an [`AnimationClip`] at a given
//! clip time, producing the mesh-local bone palette the renderer uploads (the same
//! `bind_global[j] * inverse_bind[j]` convention #79 established, but with the
//! joints' local transforms overridden by the sampled keyframes).
//!
//! Purity / determinism: every function here is a pure function of (skin, clip,
//! time) — no wall clock, no RNG — so a headless replay at the fixed timestep is
//! byte-reproducible. `glam` math only.

use crate::asset::anim_data::{AnimationClip, Interpolation, Track};
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{AnimatorComponent, MeshComponent};
use glam::{Mat4, Quat, Vec3};

/// Re-pose `mesh`'s skeleton from `anim`'s current state, writing the result into
/// the mesh's `pose_palette` (the renderer then uploads it instead of the bind
/// pose). A skinless mesh, a missing clip, or a stopped animator leaves the pose
/// palette empty so rendering falls back to the rest pose.
pub fn repose_mesh(anim: &AnimatorComponent, mesh: &mut MeshComponent) {
    let Some(skin) = clip_skin(mesh) else {
        mesh.pose_palette.clear();
        return;
    };
    let Some(current) = find_clip(mesh, &anim.current_clip) else {
        mesh.pose_palette.clear();
        return;
    };
    let current_pose = sample_palette(skin, current, anim.time);
    mesh.pose_palette = match crossfade_source(anim, mesh, skin) {
        Some(prev_pose) => blend_palettes(&prev_pose, &current_pose, anim.crossfade_weight()),
        None => current_pose,
    };
}

/// The outgoing crossfade pose, if a crossfade is active and its previous clip is
/// still resolvable on the mesh.
fn crossfade_source(
    anim: &AnimatorComponent,
    mesh: &MeshComponent,
    skin: &SkinData,
) -> Option<Vec<Mat4>> {
    if !anim.is_crossfading() {
        return None;
    }
    let prev = find_clip(mesh, anim.previous_clip.as_deref()?)?;
    Some(sample_palette(skin, prev, anim.previous_time))
}

/// The skin to pose, but only when the mesh actually carries clips to play.
fn clip_skin(mesh: &MeshComponent) -> Option<&SkinData> {
    let skin = mesh.skin.as_ref()?;
    (!mesh.clips.is_empty()).then_some(skin)
}

fn find_clip<'a>(mesh: &'a MeshComponent, name: &str) -> Option<&'a AnimationClip> {
    mesh.clips.iter().find(|c| c.name == name)
}

/// Duration of `anim`'s current clip on `mesh`, in seconds — the wrap length a
/// looping [`AnimatorComponent::advance`] needs. `0.0` when there is no mesh or the
/// clip is unresolvable (the "unknown, never wrap" sentinel `advance` expects).
pub fn current_clip_duration(anim: &AnimatorComponent, mesh: Option<&MeshComponent>) -> f32 {
    mesh.and_then(|m| find_clip(m, &anim.current_clip))
        .map_or(0.0, |c| c.duration)
}

/// Pose `skin` with `clip` sampled at `time` seconds, returning one mesh-local
/// matrix per joint slot (matching the vertices' `joint_indices`). A joint the clip
/// never animates keeps its bind-pose local transform, so a clip that drives only
/// part of the skeleton leaves the rest at rest.
pub fn sample_palette(skin: &SkinData, clip: &AnimationClip, time: f32) -> Vec<Mat4> {
    let locals = sampled_locals(skin, clip, time);
    let globals = compose_globals(skin, &locals);
    globals
        .iter()
        .zip(skin.inverse_bind.iter())
        .map(|(g, ib)| skin.mesh_inverse * *g * *ib)
        .collect()
}

/// Linearly blend two palettes of equal length, `t` in `[0, 1]` (0 → `from`,
/// 1 → `to`). Matrices are blended component-wise: adequate for the short crossfade
/// window between two skeletal poses, and fully deterministic. A length mismatch is
/// truncated to the shorter (defensive — palettes from one skin always match).
pub fn blend_palettes(from: &[Mat4], to: &[Mat4], t: f32) -> Vec<Mat4> {
    let t = t.clamp(0.0, 1.0);
    from.iter()
        .zip(to.iter())
        .map(|(a, b)| {
            let a = a.to_cols_array();
            let b = b.to_cols_array();
            let mut out = [0.0_f32; 16];
            for i in 0..16 {
                out[i] = a[i] + (b[i] - a[i]) * t;
            }
            Mat4::from_cols_array(&out)
        })
        .collect()
}

/// Sample each joint's local transform: the clip's track value at `time`, falling
/// back to the joint's bind-pose local for any untouched T/R/S path.
fn sampled_locals(skin: &SkinData, clip: &AnimationClip, time: f32) -> Vec<JointTransform> {
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

/// Compose every joint's local transform into a model-space global by walking up
/// its parent chain. The skin's `parents` form a forest (slots topologically after
/// their parents in glTF), so a single pass left-to-right resolves each global from
/// its already-resolved parent.
fn compose_globals(skin: &SkinData, locals: &[JointTransform]) -> Vec<Mat4> {
    let mut globals = vec![Mat4::IDENTITY; locals.len()];
    for (slot, local) in locals.iter().enumerate() {
        let local_mat = local.matrix();
        globals[slot] = match skin.parents.get(slot).copied().flatten() {
            // A forward reference can't happen in well-formed glTF, but guard it:
            // an unresolved parent is treated as a root rather than panicking.
            Some(parent) if parent < slot => globals[parent] * local_mat,
            _ => local_mat,
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

#[cfg(test)]
mod tests;
