//! One clip rewritten from its source rig onto a target rig (#1747).
//!
//! **Rotation** keys transfer in rest-relative space. The composition is
//! Godot 4's skeleton rest fixer (`post_import_plugin_skeleton_rest_fixer.cpp`,
//! its "fix animation by changing rest" step), with the source bone's rests as
//! the old ones and the target bone's as the new ones:
//!
//! `q_t = Pt⁻¹ · Ps · q_s · Gs⁻¹ · Gt`
//!
//! where `q_s` is the source key, `Ps`/`Pt` the global rest rotations of each
//! bone's parent (the root offset for a skeleton root) and `Gs`/`Gt` the bones'
//! own global rest rotations. It hands the bone's model-space turn away from its
//! rest (`Ps · q_s · Gs⁻¹`) to the target bone, so differing rest poses (an A-pose
//! arm onto a T-pose one) and differing bone axes carry the motion, not the
//! source's rest. A turn on a source bone the map leaves out is not carried.
//!
//! **Translations** are dropped (the target's rest lengths hold) except the
//! hips', which move by the source's offset from its rest, scaled by the target
//! over source hip height (Godot's motion scale; Unity's Humanoid does the same)
//! and carried through the parents' rests the same way: `t_t = Pt⁻¹(r · Ps(t_s −
//! rest_s)) + rest_t`. Scale keys are dropped.
//!
//! Each key is mapped on its own, at its own time: a fixed rotation on either
//! side of a slerp is a slerp of the rotated ends, and the translation map is
//! affine, so the retargeted clip samples between keys as the source would.

use glam::{Mat4, Quat, Vec3};

use super::BoneMap;
use crate::asset::anim_data::{AnimationClip, JointTrack, Track};
use crate::asset::mesh_data::SkinData;

/// A hip height under this is no height: the hips' offset is carried unscaled.
const MIN_HEIGHT: f32 = 1e-4;

/// `clip`, keyed for `src`'s joint slots, rewritten for `dst`'s, through each
/// rig's humanoid map. Joints of `dst` no mapped source bone drives keep empty
/// tracks (their rest).
pub fn retarget(
    clip: &AnimationClip,
    src: (&SkinData, &BoneMap),
    dst: (&SkinData, &BoneMap),
) -> AnimationClip {
    let (src_skin, src_map) = src;
    let (dst_skin, dst_map) = dst;
    let ratio = hip_ratio(src, dst);
    let hips = dst_map.slot("Hips");
    let mut tracks = vec![JointTrack::default(); dst_skin.local_bind.len()];
    for (t_slot, track) in tracks.iter_mut().enumerate() {
        let Some(c) = dst_map.canonical_of(t_slot) else {
            continue;
        };
        let Some(s_slot) = src_map.slots.get(c).copied().flatten() else {
            continue;
        };
        let Some(source) = clip.tracks.get(s_slot) else {
            continue;
        };
        let s = Rest::of(src_skin, s_slot);
        let t = Rest::of(dst_skin, t_slot);
        let (pre, post) = (
            t.parent_rot.inverse() * s.parent_rot,
            s.rot.inverse() * t.rot,
        );
        track.rotation = map_track(&source.rotation, |q| (pre * q * post).normalize());
        if Some(t_slot) == hips {
            let into = t.parent.inverse() * Mat4::from_scale(Vec3::splat(ratio)) * s.parent;
            let (from, to) = (s.local, t.local);
            track.translation = map_track(&source.translation, |p| {
                into.transform_vector3(p - from) + to
            });
        }
    }
    let mut out = AnimationClip {
        name: clip.name.clone(),
        tracks,
        duration: 0.0,
    };
    out.recompute_duration();
    out
}

/// A joint's rests: its own and its parent's global ones, and its local translation.
struct Rest {
    rot: Quat,
    parent: Mat4,
    parent_rot: Quat,
    local: Vec3,
}

impl Rest {
    fn of(skin: &SkinData, slot: usize) -> Self {
        let global = skin.bind_global.get(slot).copied().unwrap_or_default();
        let parent = match skin.parents.get(slot).copied().flatten() {
            Some(p) => skin.bind_global.get(p).copied().unwrap_or_default(),
            None => skin.root_offset(slot),
        };
        Self {
            rot: rotation(global),
            parent,
            parent_rot: rotation(parent),
            local: skin
                .local_bind
                .get(slot)
                .map_or(Vec3::ZERO, |l| l.translation),
        }
    }
}

fn rotation(m: Mat4) -> Quat {
    m.to_scale_rotation_translation().1.normalize()
}

/// The target over source hip height (each rig's hips above its model origin
/// at rest); 1 when either has no hips or no height.
pub(super) fn hip_ratio(src: (&SkinData, &BoneMap), dst: (&SkinData, &BoneMap)) -> f32 {
    let height = |(skin, map): (&SkinData, &BoneMap)| {
        let slot = map.slot("Hips")?;
        Some(skin.bind_global.get(slot)?.w_axis.y)
    };
    match (height(src), height(dst)) {
        (Some(s), Some(t)) if s.abs() > MIN_HEIGHT && t.abs() > MIN_HEIGHT => {
            let r = t / s;
            if r.is_finite() && r > 0.0 {
                r
            } else {
                1.0
            }
        }
        _ => 1.0,
    }
}

fn map_track<T: Copy>(track: &Track<T>, f: impl Fn(T) -> T) -> Track<T> {
    Track {
        times: track.times.clone(),
        values: track.values.iter().map(|&v| f(v)).collect(),
        interpolation: track.interpolation,
    }
}
