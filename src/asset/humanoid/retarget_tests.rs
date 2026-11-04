//! The retarget (#1747) on primitive rigs whose heights, bone axes, rest poses
//! and root offsets differ.

use glam::{Mat4, Quat, Vec3};

use super::rig_fixture::{posed, rig, walk, Shape, BONES, HIPS, LEFT_FOOT, LEFT_LOWER_ARM};
use super::{retarget, BoneMap, HumanoidSettings};
use crate::asset::mesh_data::SkinData;

fn taller() -> Shape {
    Shape {
        height: 1.2,
        axes: Quat::from_euler(glam::EulerRot::XYZ, 0.7, -0.4, 1.1),
        arm_down: 0.0,
        root_offset: Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2),
    }
}

fn moved(a: &SkinData, b: &SkinData) -> crate::asset::anim_data::AnimationClip {
    let none = HumanoidSettings::default();
    let (ma, mb) = (BoneMap::resolve(a, &none), BoneMap::resolve(b, &none));
    retarget(&walk(a), (a, &ma), (b, &mb))
}

fn at(m: &Mat4) -> Vec3 {
    m.w_axis.truncate()
}

#[test]
fn a_walk_on_a_taller_rig_keeps_its_feet_and_its_limb_lengths() {
    let (a, b) = (rig(Shape::default()), rig(taller()));
    let (clip_a, clip_b) = (walk(&a), moved(&a, &b));
    for k in 0..5 {
        let (pa, pb) = (posed(&a, &clip_a, k), posed(&b, &clip_b, k));
        let foot = at(&pb[LEFT_FOOT]) - 1.2 * at(&pa[LEFT_FOOT]);
        assert!(foot.length() < 1e-3, "key {k}: the foot is {foot} off");
        for (slot, (_, parent, _)) in BONES.iter().enumerate() {
            let Some(p) = parent else { continue };
            let rest = (at(&b.bind_global[slot]) - at(&b.bind_global[*p])).length();
            let now = (at(&pb[slot]) - at(&pb[*p])).length();
            assert!(
                (now - rest).abs() < 1e-4,
                "key {k}: bone {slot} is {now}, rest {rest}"
            );
        }
    }
}

#[test]
fn the_hips_travel_scales_with_the_hip_height() {
    let (a, b) = (rig(Shape::default()), rig(taller()));
    let (clip_a, clip_b) = (walk(&a), moved(&a, &b));
    let travel = |s: &SkinData, c, k| at(&posed(s, c, k)[HIPS]);
    let da = travel(&a, &clip_a, 4) - travel(&a, &clip_a, 0);
    let db = travel(&b, &clip_b, 4) - travel(&b, &clip_b, 0);
    assert!((db - 1.2 * da).length() < 1e-4, "{db} vs 1.2 × {da}");
}

/// Each bone's model-space turn away from its rest, the quantity the
/// retarget hands over: it holds across an A-pose and a T-pose arm, where a
/// multiply on the wrong side of the rests would not.
#[test]
fn an_a_pose_arm_takes_the_t_pose_arms_turn() {
    let a = rig(Shape::default());
    let b = rig(Shape {
        arm_down: 0.6,
        ..taller()
    });
    let (clip_a, clip_b) = (walk(&a), moved(&a, &b));
    let delta = |s: &SkinData, g: &Mat4, slot: usize| {
        let rot = |m: &Mat4| m.to_scale_rotation_translation().1;
        rot(g) * rot(&s.bind_global[slot]).inverse()
    };
    for k in 0..5 {
        let (pa, pb) = (posed(&a, &clip_a, k), posed(&b, &clip_b, k));
        for slot in 0..BONES.len() {
            let (da, db) = (delta(&a, &pa[slot], slot), delta(&b, &pb[slot], slot));
            assert!(
                da.dot(db).abs() > 1.0 - 1e-5,
                "key {k} bone {slot}: {da} vs {db}"
            );
        }
    }
    let lower = clip_b.tracks[LEFT_LOWER_ARM].rotation.values.len();
    assert_eq!(lower, 5, "keys keep their times");
}

#[test]
fn a_bone_the_target_map_leaves_out_keeps_its_rest() {
    let a = rig(Shape::default());
    let b = rig(taller());
    let mut cut = HumanoidSettings::default();
    cut.bones.insert("LeftLowerLeg".into(), String::new());
    let (ma, mb) = (
        BoneMap::resolve(&a, &HumanoidSettings::default()),
        BoneMap::resolve(&b, &cut),
    );
    let clip = retarget(&walk(&a), (&a, &ma), (&b, &mb));
    assert!(clip.tracks[4].is_empty());
    assert!(
        clip.tracks[3].translation.is_empty(),
        "a limb's translation is dropped"
    );
    assert_eq!(clip.tracks.len(), b.local_bind.len());
    assert_eq!(clip.duration, 1.0);
}

#[test]
fn rigs_without_hip_height_carry_the_hips_unscaled() {
    let flat = |h: f32| {
        let mut s = rig(Shape {
            height: h,
            ..Shape::default()
        });
        s.bind_global[HIPS].w_axis.y = 0.0;
        s
    };
    let none = HumanoidSettings::default();
    let (a, b) = (flat(1.0), flat(2.0));
    let (ma, mb) = (BoneMap::resolve(&a, &none), BoneMap::resolve(&b, &none));
    assert_eq!(super::retarget::hip_ratio((&a, &ma), (&b, &mb)), 1.0);
}
