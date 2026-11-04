//! The retarget (#1747) on primitive rigs whose heights, bone axes, rest poses,
//! root offsets, armatures and facing differ. Every position is model space.

use glam::{Mat4, Quat, Vec3};

use super::rig_fixture::{
    blender_armature, model_rest, posed, rig, walk, Shape, BONES, HIPS, LEFT_FOOT,
};
use super::{retarget, BoneMap, HumanoidSettings};
use crate::asset::anim_data::AnimationClip;
use crate::asset::mesh_data::SkinData;

fn taller() -> Shape {
    Shape {
        height: 1.2,
        axes: Quat::from_euler(glam::EulerRot::XYZ, 0.7, -0.4, 1.1),
        root_offset: Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        ..Shape::default()
    }
}

fn exported(shape: Shape) -> Shape {
    Shape {
        armature: blender_armature(),
        ..shape
    }
}

/// (source, target) pairs: native glTF rigs, a Blender export as the target,
/// as the source, and on both sides, and a target facing −Z.
pub(super) fn pairings() -> Vec<(Shape, Shape)> {
    let plain = Shape::default();
    vec![
        (plain, taller()),
        (plain, exported(taller())),
        (exported(plain), taller()),
        (exported(plain), exported(taller())),
        (
            plain,
            Shape {
                facing: std::f32::consts::PI,
                ..exported(taller())
            },
        ),
    ]
}

pub(super) fn moved(a: &SkinData, b: &SkinData) -> AnimationClip {
    let none = HumanoidSettings::default();
    let (ma, mb) = (BoneMap::resolve(a, &none), BoneMap::resolve(b, &none));
    retarget(&walk(a), (a, &ma), (b, &mb))
}

fn at(m: &Mat4) -> Vec3 {
    m.w_axis.truncate()
}

#[test]
fn a_walk_on_a_taller_rig_keeps_its_feet_and_its_limb_lengths() {
    for (n, (sa, sb)) in pairings().into_iter().enumerate() {
        let (a, b) = (rig(sa), rig(sb));
        let (clip_a, clip_b) = (walk(&a), moved(&a, &b));
        let turn = Quat::from_rotation_y(sb.facing - sa.facing);
        for k in 0..5 {
            let (pa, pb) = (posed(&a, &clip_a, k), posed(&b, &clip_b, k));
            let foot = at(&pb[LEFT_FOOT]) - turn * (1.2 * at(&pa[LEFT_FOOT]));
            assert!(foot.length() < 1e-3, "pair {n} key {k}: foot {foot} off");
            for (slot, (_, parent, _)) in BONES.iter().enumerate() {
                let Some(p) = parent else { continue };
                let rest = (at(&model_rest(&b, slot)) - at(&model_rest(&b, *p))).length();
                let now = (at(&pb[slot]) - at(&pb[*p])).length();
                assert!(
                    (now - rest).abs() < 1e-4,
                    "pair {n} key {k}: bone {slot} is {now}, rest {rest}"
                );
            }
        }
    }
}

#[test]
fn the_hips_travel_scales_with_the_hip_height() {
    for (n, (sa, sb)) in pairings().into_iter().enumerate() {
        let (a, b) = (rig(sa), rig(sb));
        let (clip_a, clip_b) = (walk(&a), moved(&a, &b));
        let travel = |s: &SkinData, c, k| at(&posed(s, c, k)[HIPS]);
        let da = travel(&a, &clip_a, 4) - travel(&a, &clip_a, 0);
        let db = travel(&b, &clip_b, 4) - travel(&b, &clip_b, 0);
        let want = Quat::from_rotation_y(sb.facing - sa.facing) * (1.2 * da);
        assert!(
            (da - Vec3::Z).length() < 1e-4,
            "pair {n}: the source walks +Z"
        );
        assert!((db - want).length() < 1e-4, "pair {n}: {db} vs {want}");
        let ratio = super::retarget::hip_ratio(
            (&a, &BoneMap::resolve(&a, &HumanoidSettings::default())),
            (&b, &BoneMap::resolve(&b, &HumanoidSettings::default())),
        );
        assert!((ratio - 1.2).abs() < 1e-4, "pair {n}: hip ratio {ratio}");
    }
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
fn a_borrowed_clip_keeps_the_sources_duration() {
    let (a, b) = (rig(Shape::default()), rig(taller()));
    let none = HumanoidSettings::default();
    let mut long = walk(&a);
    long.duration = 2.0;
    let (ma, mb) = (BoneMap::resolve(&a, &none), BoneMap::resolve(&b, &none));
    assert_eq!(retarget(&long, (&a, &ma), (&b, &mb)).duration, 2.0);
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
