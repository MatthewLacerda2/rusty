//! What the grading effects pack for the bake (#1440, #1441): neutral without a
//! volume, the ops' clamps held again for a loaded file, Unity's white balance and
//! Lift / Gamma / Gain preparation.

use glam::Vec3;

use super::super::{grade_layout, grade_slots, GradeCx};
use crate::components::{ColorWheel, PostProcessingComponent, Tonemap, WhiteBalance};
use crate::scene::authoring::default_post_processing;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).abs().max_element() < 1e-3
}

/// `effect`'s grade slots for `volume` (and a lookup).
fn packed(
    volume: Option<&PostProcessingComponent>,
    lookup: Option<([[f32; 3]; 2], f32)>,
    effect: &str,
) -> Vec<[f32; 4]> {
    let all = grade_slots(&GradeCx { volume, lookup });
    grade_layout()
        .iter()
        .filter(|p| p.effect.name == effect)
        .flat_map(|p| all[p.offset..p.offset + p.function.slots].to_vec())
        .collect()
}

#[test]
fn neutral_knobs_bake_the_identity_grade_with_the_volumes_tonemap() {
    let vc = default_post_processing();
    let v = Some(&vc);
    assert_eq!(
        packed(v, None, "white_balance"),
        [[1.0, 1.0, 1.0, 0.0]],
        "skipped"
    );
    let lgg = packed(v, None, "lift_gamma_gain");
    assert!(close(Vec3::from_slice(&lgg[0][..3]), Vec3::ZERO));
    assert!(close(Vec3::from_slice(&lgg[1][..3]), Vec3::ONE));
    assert!(close(Vec3::from_slice(&lgg[2][..3]), Vec3::ONE));
    let adjust = packed(v, None, "color_adjustments");
    let neutral = [1.0, 0.0, 0.0, 0.0];
    assert_eq!(
        adjust,
        [neutral, [0.0; 4], neutral],
        "contrast, hue, saturation"
    );
    let tone = packed(v, None, "tonemapping");
    assert_eq!(
        tone,
        [
            [Tonemap::Aces.to_index() as f32, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0]
        ]
    );
}

#[test]
fn no_volume_bakes_the_neutral_grade_without_a_tonemap() {
    let lgg = packed(None, None, "lift_gamma_gain");
    assert_eq!(
        lgg,
        [[0.0; 4], [1.0, 1.0, 1.0, 0.0], [1.0, 1.0, 1.0, 0.0]],
        "exact"
    );
    assert_eq!(
        packed(None, None, "tonemapping")[0][0],
        Tonemap::None.to_index() as f32
    );
    assert_eq!(
        packed(None, None, "color_lookup"),
        [[0.0; 4], [1.0, 1.0, 1.0, 0.0]]
    );
}

#[test]
fn knobs_a_file_left_out_of_range_bake_as_the_ops_would_clamp_them() {
    let mut wild = default_post_processing();
    wild.white_balance.temperature = 300.0;
    wild.color_adjustments.hue_shift = 400.0;
    wild.lift_gamma_gain.gain = ColorWheel {
        color: [2.0, 0.5, -1.0],
        offset: 5.0,
    };
    let mut held = default_post_processing();
    held.white_balance.temperature = WhiteBalance::RANGE;
    held.color_adjustments.hue_shift = 180.0;
    held.lift_gamma_gain.gain = ColorWheel {
        color: [1.0, 0.5, 0.0],
        offset: 1.0,
    };
    let bake = |vc: &PostProcessingComponent| {
        grade_slots(&GradeCx {
            volume: Some(vc),
            lookup: None,
        })
    };
    assert_eq!(bake(&wild), bake(&held));
    wild.white_balance.tint = f32::NAN;
    assert_eq!(bake(&wild), bake(&held), "NaN is neutral");
}

#[test]
fn a_lookup_is_off_until_one_is_applied_and_holds_its_contribution_to_unit() {
    let vc = default_post_processing();
    assert_eq!(
        packed(Some(&vc), None, "color_lookup")[0][3],
        0.0,
        "none by default"
    );
    let domain = [[0.0; 3], [2.0; 3]];
    let applied = packed(Some(&vc), Some((domain, 3.0)), "color_lookup");
    assert_eq!((applied[0], applied[1][0]), ([0.0, 0.0, 0.0, 1.0], 2.0));
    let nan = packed(Some(&vc), Some((domain, f32::NAN)), "color_lookup");
    assert_eq!(nan[0][3], 0.0);
}
