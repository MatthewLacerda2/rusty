//! Unity's white balance and Lift / Gamma / Gain preparation (#1440), as the grading
//! effects pack them.

use glam::Vec3;

use super::lift_gamma_gain::prepare;
use super::white_balance::coefficients;
use crate::components::{ColorWheel, LiftGammaGain, WhiteBalance};

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).abs().max_element() < 1e-3
}

#[test]
fn a_positive_temperature_scales_long_wavelengths_over_short() {
    let warm = coefficients(WhiteBalance {
        temperature: 50.0,
        tint: 0.0,
    });
    assert!(warm.x > warm.z, "L over S: {warm}");
    let cool = coefficients(WhiteBalance {
        temperature: -50.0,
        tint: 0.0,
    });
    assert!(cool.z > cool.x, "S over L: {cool}");
    // D65 sits on the locus formula: the neutral coefficients are ~1 even if the
    // shortcut that skips them were removed.
    let d65 = coefficients(WhiteBalance {
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
    let (lift, inv_gamma, gain) = prepare(&lgg);
    assert!(close(lift, Vec3::splat(0.2)), "{lift}");
    assert!(close(gain, Vec3::splat(1.5)), "{gain}");
    assert!(close(inv_gamma, Vec3::splat(0.5)), "{inv_gamma}");
    lgg.gain = ColorWheel {
        color: [1.0, 0.0, 0.0],
        offset: 0.0,
    };
    let (_, _, gain) = prepare(&lgg);
    assert!(
        gain.x > 1.0 && gain.y < 1.0 && gain.z < 1.0,
        "a red gain: {gain}"
    );
}
