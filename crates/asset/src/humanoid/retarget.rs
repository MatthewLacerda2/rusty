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
//! **Both rigs' rests are read in their body frame** ([`body`]): model space
//! (the skinned mesh node's own transform applied, so an `Armature` node's
//! rotation and scale, Blender's Z-up and centimetres, are undone and up is +Y as
//! glTF has it), turned about +Y so the body faces +Z, as Unity's Humanoid
//! normalises. So a rig exported Z-up, or facing −Z, takes a walk forward and up.
//!
//! Each key is mapped on its own, at its own time: a fixed rotation on either
//! side of a slerp is a slerp of the rotated ends, and the translation map is
//! affine, so the retargeted clip samples between keys as the source would.

use glam::{Mat4, Quat, Vec3};

use super::BoneMap;
use crate::anim_data::{AnimationClip, JointTrack, Track};
use crate::mesh_data::SkinData;

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
    let (src_body, dst_body) = (body(src_skin, src_map), body(dst_skin, dst_map));
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
        let s = Rest::of(src_skin, s_slot, src_body);
        let t = Rest::of(dst_skin, t_slot, dst_body);
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
    // The source's length, not the kept tracks': wrap, phase and events land
    // where they do on the source rig.
    AnimationClip {
        name: clip.name.clone(),
        tracks,
        duration: clip.duration,
    }
}

/// A joint's rests: its own and its parent's, in its rig's body frame, and its
/// local translation.
struct Rest {
    rot: Quat,
    parent: Mat4,
    parent_rot: Quat,
    local: Vec3,
}

impl Rest {
    fn of(skin: &SkinData, slot: usize, body: Mat4) -> Self {
        let global = body * skin.bind_global.get(slot).copied().unwrap_or_default();
        let parent = body
            * match skin.parents.get(slot).copied().flatten() {
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
    // lint: allow(world-space) a skin's bind matrices, rig data, not the scene hierarchy
    m.to_scale_rotation_translation().1.normalize()
}

/// The target over source hip height (each rig's hips above its model origin
/// at rest, along model +Y); 1 when either has no hips or no height.
pub(super) fn hip_ratio(src: (&SkinData, &BoneMap), dst: (&SkinData, &BoneMap)) -> f32 {
    let height = |(skin, map): (&SkinData, &BoneMap)| {
        let slot = map.slot("Hips")?;
        Some((body(skin, map) * *skin.bind_global.get(slot)?).w_axis.y)
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

/// From `skin`'s mesh-local space into its body frame: model space (the mesh
/// node's transform, [`SkinData::mesh_to_model`]), then turned about +Y so the
/// body faces +Z. Its facing is the ground-plane direction its left side points
/// (glTF: a model faces +Z, its left on +X), read from the first left/right pair
/// the map has; a rig without one keeps model space's facing.
pub(super) fn body(skin: &SkinData, map: &BoneMap) -> Mat4 {
    let model = skin.mesh_to_model();
    let at = |slot: usize| {
        let rest = skin.bind_global.get(slot).copied().unwrap_or_default();
        (model * rest).w_axis.truncate()
    };
    let pairs = [
        ("LeftUpperLeg", "RightUpperLeg"),
        ("LeftUpperArm", "RightUpperArm"),
        ("LeftShoulder", "RightShoulder"),
        ("LeftFoot", "RightFoot"),
    ];
    let left = pairs.iter().find_map(|(l, r)| {
        let v = at(map.slot(l)?) - at(map.slot(r)?);
        let v = Vec3::new(v.x, 0.0, v.z);
        (v.is_finite() && v.length() > MIN_HEIGHT).then_some(v)
    });
    let Some(left) = left else { return model };
    let forward = left.cross(Vec3::Y);
    let facing = Quat::from_rotation_y(forward.x.atan2(forward.z));
    Mat4::from_quat(facing.inverse()) * model
}

fn map_track<T: Copy>(track: &Track<T>, f: impl Fn(T) -> T) -> Track<T> {
    Track {
        times: track.times.clone(),
        values: track.values.iter().map(|&v| f(v)).collect(),
        interpolation: track.interpolation,
    }
}
