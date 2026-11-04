//! Primitive humanoid rigs for the retarget tests (#1747): built from mesh-space
//! rests, so two rigs can share a silhouette while their bone axes, rest poses,
//! root offsets and heights differ.

use glam::{Mat4, Quat, Vec3};

use crate::asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use crate::asset::mesh_data::{JointTransform, SkinData};

/// (name, parent, rest position in mesh space at height 1, T-pose).
pub const BONES: &[(&str, Option<usize>, [f32; 3])] = &[
    ("mixamorig:Hips", None, [0.0, 0.9, 0.0]),
    ("mixamorig:Spine", Some(0), [0.0, 1.1, 0.0]),
    ("mixamorig:Head", Some(1), [0.0, 1.5, 0.0]),
    ("mixamorig:LeftUpLeg", Some(0), [0.1, 0.85, 0.0]),
    ("mixamorig:LeftLeg", Some(3), [0.1, 0.45, 0.0]),
    ("mixamorig:LeftFoot", Some(4), [0.1, 0.05, 0.0]),
    ("mixamorig:RightUpLeg", Some(0), [-0.1, 0.85, 0.0]),
    ("mixamorig:RightLeg", Some(6), [-0.1, 0.45, 0.0]),
    ("mixamorig:RightFoot", Some(7), [-0.1, 0.05, 0.0]),
    ("mixamorig:LeftArm", Some(1), [0.2, 1.4, 0.0]),
    ("mixamorig:LeftForeArm", Some(9), [0.5, 1.4, 0.0]),
];
pub const HIPS: usize = 0;
pub const LEFT_FOOT: usize = 5;
pub const LEFT_UPPER_ARM: usize = 9;
pub const LEFT_LOWER_ARM: usize = 10;

/// How a rig differs from the plain one.
#[derive(Clone, Copy)]
pub struct Shape {
    pub height: f32,
    /// Each bone's rest axes turned by this, varied per slot (a different export).
    pub axes: Quat,
    /// The arm lowered by this many radians about +Z at the shoulder (an A-pose).
    pub arm_down: f32,
    /// The skeleton root's parent space (an `Armature` node).
    pub root_offset: Mat4,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            height: 1.0,
            axes: Quat::IDENTITY,
            arm_down: 0.0,
            root_offset: Mat4::IDENTITY,
        }
    }
}

/// The rig `shape` describes.
pub fn rig(shape: Shape) -> SkinData {
    let down = Quat::from_rotation_z(-shape.arm_down);
    let shoulder = Vec3::from(BONES[LEFT_UPPER_ARM].2) * shape.height;
    let globals: Vec<Mat4> = (BONES.iter().enumerate())
        .map(|(slot, (_, _, at))| {
            let mut p = Vec3::from(*at) * shape.height;
            let mut r = shape.axes.slerp(Quat::IDENTITY, slot as f32 / 20.0);
            if slot == LEFT_UPPER_ARM || slot == LEFT_LOWER_ARM {
                p = shoulder + down * (p - shoulder);
                r = down * r;
            }
            Mat4::from_rotation_translation(r, p)
        })
        .collect();
    let local_bind = (BONES.iter().enumerate())
        .map(|(slot, (_, parent, _))| {
            let parent = parent.map_or(shape.root_offset, |p| globals[p]);
            let (scale, rotation, translation) =
                (parent.inverse() * globals[slot]).to_scale_rotation_translation();
            JointTransform {
                translation,
                rotation,
                scale,
            }
        })
        .collect();
    SkinData {
        inverse_bind: globals.iter().map(|g| g.inverse()).collect(),
        bind_global: globals,
        local_bind,
        parents: BONES.iter().map(|b| b.1).collect(),
        joint_nodes: (0..BONES.len()).collect(),
        mesh_inverse: Mat4::IDENTITY,
        names: BONES.iter().map(|b| b.0.to_string()).collect(),
    }
}

/// Five keys of a walk on `skin`: the hips bob and travel 1 m along +Z, the legs
/// and the arm swing, each key a turn on top of the bone's rest local.
pub fn walk(skin: &SkinData) -> AnimationClip {
    let times = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    let swing = [0.0, 0.5, 0.0, -0.5, 0.0];
    let mut tracks = vec![JointTrack::default(); BONES.len()];
    let rest = |slot: usize| skin.local_bind[slot];
    let turn = |slot: usize, axis: Vec3, k: f32| {
        let values = swing
            .iter()
            .map(|s| rest(slot).rotation * Quat::from_axis_angle(axis, s * k));
        Track {
            times: times.clone(),
            values: values.collect(),
            interpolation: Interpolation::Linear,
        }
    };
    tracks[HIPS].rotation = turn(HIPS, Vec3::Y, 0.2);
    tracks[HIPS].translation = Track {
        times: times.clone(),
        values: (0..5)
            .map(|k| {
                rest(HIPS).translation + Vec3::new(0.0, -0.03 * swing[k].abs(), 0.25 * k as f32)
            })
            .collect(),
        interpolation: Interpolation::Linear,
    };
    for (slot, k) in [
        (3, 1.0),
        (4, -0.8),
        (6, -1.0),
        (7, 0.8),
        (9, 0.6),
        (10, 0.4),
    ] {
        tracks[slot].rotation = turn(slot, Vec3::X, k);
        // A translation key on a limb, the source's own length: the retarget
        // drops it, so the target's length holds.
        tracks[slot].translation = Track {
            times: vec![0.0],
            values: vec![rest(slot).translation],
            interpolation: Interpolation::Linear,
        };
    }
    let mut clip = AnimationClip {
        name: "Walk".into(),
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

/// `skin`'s mesh-space joint globals at `clip`'s key `k` (every keyed track
/// has a value there, or one held), rests where unkeyed.
pub fn posed(skin: &SkinData, clip: &AnimationClip, k: usize) -> Vec<Mat4> {
    let mut globals: Vec<Mat4> = Vec::with_capacity(skin.parents.len());
    for slot in 0..skin.parents.len() {
        let mut local = skin.local_bind[slot];
        let track = &clip.tracks[slot];
        let key = |n: usize| k.min(n.saturating_sub(1));
        if let Some(t) = track
            .translation
            .values
            .get(key(track.translation.values.len()))
        {
            local.translation = *t;
        }
        if let Some(r) = track.rotation.values.get(key(track.rotation.values.len())) {
            local.rotation = *r;
        }
        let parent = skin.parents[slot].map_or(skin.root_offset(slot), |p| globals[p]);
        globals.push(parent * local.matrix());
    }
    globals
}
