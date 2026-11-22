//! Rotation keys transfer in rest-relative space, as in Godot 4's skeleton rest fixer:
//! `q_t = Pt⁻¹ · Ps · q_s · Gs⁻¹ · Gt` (`P` a parent's global rest, `G` the bone's own).
//! Translations are dropped except the hips', scaled by target over source hip height (as
//! Godot and Unity do). Rests are read in each rig's body frame ([`body`]): model space
//! turned about +Y so the body faces +Z, as Unity's Humanoid normalises.

use glam::{Mat4, Quat, Vec3};

use super::BoneMap;
use crate::anim_data::{AnimationClip, JointTrack, Track};
use crate::mesh_data::SkinData;

/// Model units: a hip height or left/right gap below this counts as none.
const MIN_HEIGHT: f32 = 1e-4;

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
    // The source's length, so wrap and events land as on the source rig.
    AnimationClip {
        name: clip.name.clone(),
        tracks,
        duration: clip.duration,
    }
}

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
