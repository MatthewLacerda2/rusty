//! The retarget's rotation transfer (#1747) across rest poses, armatures and
//! facing: each bone's turn away from its rest, in each rig's body frame.

use glam::{Mat4, Quat};

use super::retarget_tests::{moved, pairings};
use super::rig_fixture::{model_rest, posed, rig, walk, Shape, BONES, LEFT_LOWER_ARM};
use crate::asset::mesh_data::SkinData;

/// It holds across an A-pose and a T-pose arm, a Z-up export and a rig facing
/// the other way, where a multiply on the wrong side of the rests would not.
#[test]
fn every_bone_takes_the_sources_turn_in_the_body_frame() {
    for (n, (sa, sb)) in pairings().into_iter().enumerate() {
        let sb = Shape {
            arm_down: 0.6,
            ..sb
        };
        let (a, b) = (rig(sa), rig(sb));
        let (clip_a, clip_b) = (walk(&a), moved(&a, &b));
        let turn = |s: &Shape| Quat::from_rotation_y(s.facing);
        let delta = |s: &SkinData, g: &Mat4, slot: usize| {
            let rot = |m: &Mat4| m.to_scale_rotation_translation().1;
            rot(g) * rot(&model_rest(s, slot)).inverse()
        };
        for k in 0..5 {
            let (pa, pb) = (posed(&a, &clip_a, k), posed(&b, &clip_b, k));
            for slot in 0..BONES.len() {
                let da = turn(&sa).inverse() * delta(&a, &pa[slot], slot) * turn(&sa);
                let db = turn(&sb).inverse() * delta(&b, &pb[slot], slot) * turn(&sb);
                assert!(
                    da.dot(db).abs() > 1.0 - 1e-5,
                    "pair {n} key {k} bone {slot}: {da} vs {db}"
                );
            }
        }
        let lower = clip_b.tracks[LEFT_LOWER_ARM].rotation.values.len();
        assert_eq!(lower, 5, "keys keep their times");
    }
}
