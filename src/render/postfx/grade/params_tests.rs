use glam::Vec3;

use super::{lift_gamma_gain, white_balance, GradeParams};
use crate::components::{ColorWheel, LiftGammaGain, Tonemap, WhiteBalance};
use crate::scene::authoring::default_post_processing;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).abs().max_element() < 1e-3
}

#[test]
fn neutral_knobs_bake_the_identity_grade_with_the_volumes_tonemap() {
    let vc = default_post_processing();
    let p = GradeParams::from_volume(Some(&vc));
    assert_eq!(
        p.white_balance,
        [1.0, 1.0, 1.0, 0.0],
        "white balance skipped"
    );
    assert!(close(Vec3::from_slice(&p.lift), Vec3::ZERO));
    assert!(close(Vec3::from_slice(&p.inv_gamma), Vec3::ONE));
    assert!(close(Vec3::from_slice(&p.gain), Vec3::ONE));
    assert_eq!(p.hue[0], 0.0);
    assert_eq!(p.tone, [Tonemap::Aces.to_index() as f32, 1.0, 1.0, 1.0]);
    assert_eq!(GradeParams::from_volume(None), GradeParams::NEUTRAL);
}

#[test]
fn a_positive_temperature_scales_long_wavelengths_over_short() {
    let warm = white_balance(WhiteBalance {
        temperature: 50.0,
        tint: 0.0,
    });
    assert!(warm.x > warm.z, "L over S: {warm}");
    let cool = white_balance(WhiteBalance {
        temperature: -50.0,
        tint: 0.0,
    });
    assert!(cool.z > cool.x, "S over L: {cool}");
    // D65 sits on the locus formula: the neutral coefficients are ~1 even if the
    // shortcut that skips them were removed.
    let d65 = white_balance(WhiteBalance {
        temperature: 1e-6,
        tint: 0.0,
    });
    assert!(close(d65, Vec3::ONE), "{d65}");
}

#[test]
fn a_grey_wheel_tints_nothing_and_its_offset_moves_every_channel() {
    let mut lgg = LiftGammaGain {
        lift: ColorWheel {
            color: [0.5; 3],
            offset: 0.2,
        },
        ..Default::default()
    };
    lgg.gain.offset = 0.5;
    lgg.gamma.offset = 1.0;
    let (lift, inv_gamma, gain) = lift_gamma_gain(&lgg);
    assert!(close(lift, Vec3::splat(0.2)), "{lift}");
    assert!(close(gain, Vec3::splat(1.5)), "{gain}");
    assert!(close(inv_gamma, Vec3::splat(0.5)), "{inv_gamma}");

    lgg.gain = ColorWheel {
        color: [1.0, 0.0, 0.0],
        offset: 0.0,
    };
    let (_, _, gain) = lift_gamma_gain(&lgg);
    assert!(
        gain.x > 1.0 && gain.y < 1.0 && gain.z < 1.0,
        "a red gain: {gain}"
    );
}

#[test]
fn knobs_a_file_left_out_of_range_bake_as_the_ops_would_clamp_them() {
    let mut wild = default_post_processing();
    wild.white_balance.temperature = 300.0;
    wild.hue_shift = 400.0;
    wild.lift_gamma_gain.gain = ColorWheel {
        color: [2.0, 0.5, -1.0],
        offset: 5.0,
    };
    let mut held = default_post_processing();
    held.white_balance.temperature = WhiteBalance::RANGE;
    held.hue_shift = 180.0;
    held.lift_gamma_gain.gain = ColorWheel {
        color: [1.0, 0.5, 0.0],
        offset: 1.0,
    };
    let held = GradeParams::from_volume(Some(&held));
    assert_eq!(GradeParams::from_volume(Some(&wild)), held);
    wild.white_balance.tint = f32::NAN;
    assert_eq!(
        GradeParams::from_volume(Some(&wild)),
        held,
        "NaN is neutral"
    );
}

#[test]
fn a_lookup_is_off_until_one_is_applied_and_holds_its_contribution_to_unit() {
    let vc = default_post_processing();
    let graded = GradeParams::from_volume(Some(&vc));
    assert_eq!(graded.lookup_min[3], 0.0, "no LUT applies by default");
    let domain = [[0.0; 3], [2.0; 3]];
    let applied = graded.with_lookup(domain, 3.0);
    assert_eq!(
        (applied.lookup_min, applied.lookup_max[0]),
        ([0.0, 0.0, 0.0, 1.0], 2.0)
    );
    assert_eq!(graded.with_lookup(domain, f32::NAN).lookup_min[3], 0.0);
}
