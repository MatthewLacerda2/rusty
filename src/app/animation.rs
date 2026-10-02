//! src/app/animation.rs — the deterministic keyframe sampler (#80).
//!
//! Poses an imported skeleton ([`SkinData`]) from an [`AnimationClip`] at a given
//! clip time. Since #453 the pose lands on the **bone GameObjects**: the sampler
//! produces each joint's local TRS, a crossfade blends two poses in TRS space
//! (lerp translation and scale, slerp rotation — blending finished matrices
//! shrinks limbs), and the `animate` system writes the result onto the bones. The
//! skin palette is then built from the bones by `Scene::build_skin_palettes`, after
//! every later writer (scripts, physics) had its say.
//!
//! Purity / determinism: every function here is a pure function of (skin, clip,
//! time) — no wall clock, no RNG — so a headless replay at the fixed timestep is
//! byte-reproducible. `glam` math only.

pub(crate) mod graph;

use crate::asset::anim_data::{AnimationClip, Interpolation, Track};
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{AnimatorComponent, MeshComponent, TransformComponent};
use glam::{Mat4, Quat, Vec3};

/// The local transform `anim` gives each of `mesh`'s bones this tick, as
/// `(bone entity, transform)` writes. A joint neither the current nor the
/// outgoing clip animates is not written, so a script or an override posing it
/// keeps it (Unity leaves unanimated properties alone). Empty without a bound
/// skeleton or a resolvable clip: a stopped animator leaves the bones where they
/// are.
pub fn bone_writes(
    anim: &AnimatorComponent,
    mesh: &MeshComponent,
) -> Vec<(u32, TransformComponent)> {
    let Some((skin, pose)) = sample_pose(anim, mesh) else {
        return Vec::new();
    };
    let bones = &mesh.skeleton.bones;
    if bones.len() != pose.len() {
        return Vec::new();
    }
    pose.into_iter()
        .enumerate()
        .filter_map(|(slot, local)| Some((bones[slot], bone_local(skin, slot, local?))))
        .collect()
}

/// A joint's sampled local as its bone's Transform. A skeleton root's bone hangs
/// off the skinned entity, so the fixed root offset (an `Armature` node) is folded
/// in; every other bone's parent is its parent joint's bone, so TRS maps 1:1.
fn bone_local(skin: &SkinData, slot: usize, local: JointTransform) -> TransformComponent {
    if skin.parents.get(slot).copied().flatten().is_some() {
        return TransformComponent {
            position: local.translation,
            rotation: local.rotation,
            scale: local.scale,
        };
    }
    TransformComponent::from_matrix(skin.root_offset(slot) * local.matrix())
}

/// The pose `anim` puts `mesh`'s skeleton in: per joint slot, the local TRS, or
/// `None` for a joint no playing clip animates. Crossfades blend in TRS space.
/// `None` for a skinless mesh or an unresolvable clip.
pub fn sample_pose<'m>(
    anim: &AnimatorComponent,
    mesh: &'m MeshComponent,
) -> Option<(&'m SkinData, Vec<Option<JointTransform>>)> {
    let skin = clip_skin(mesh)?;
    let current = find_clip(mesh, &anim.current_clip)?;
    let mut pose = sampled_locals(skin, current, anim.time);
    let mut driven = driven_slots(current, pose.len());
    if let Some(prev) = crossfade_source(anim, mesh) {
        let from = sampled_locals(skin, prev, anim.previous_time);
        pose = blend_poses(&from, &pose, anim.crossfade_weight());
        for (d, p) in driven.iter_mut().zip(driven_slots(prev, pose.len())) {
            *d |= p;
        }
    }
    let pose = pose
        .into_iter()
        .zip(driven)
        .map(|(local, driven)| driven.then_some(local))
        .collect();
    Some((skin, pose))
}

/// Which joint slots `clip` has any keyframes for.
fn driven_slots(clip: &AnimationClip, joints: usize) -> Vec<bool> {
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

/// The clip being faded out, if a crossfade is active and its clip is still
/// resolvable on the mesh.
fn crossfade_source<'m>(
    anim: &AnimatorComponent,
    mesh: &'m MeshComponent,
) -> Option<&'m AnimationClip> {
    if !anim.is_crossfading() {
        return None;
    }
    find_clip(mesh, anim.previous_clip.as_deref()?)
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

/// Blend two poses of equal length in TRS space, `t` in `[0, 1]` (0 → `from`,
/// 1 → `to`): translation and scale lerp, rotation slerps along the shortest arc,
/// so a limb keeps its length however far apart the two poses are. A length
/// mismatch is truncated to the shorter (poses from one skin always match).
pub fn blend_poses(from: &[JointTransform], to: &[JointTransform], t: f32) -> Vec<JointTransform> {
    let t = t.clamp(0.0, 1.0);
    from.iter()
        .zip(to.iter())
        .map(|(a, b)| JointTransform {
            translation: a.translation.lerp(b.translation, t),
            rotation: a.rotation.slerp(b.rotation, t),
            scale: a.scale.lerp(b.scale, t),
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

/// Compose every joint's local transform into a mesh-local global by walking up
/// its parent chain (a root picks up its [`SkinData::root_offset`]). The skin's `parents` form a forest (slots topologically after
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

#[cfg(test)]
mod tests;
