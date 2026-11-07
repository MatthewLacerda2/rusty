//! Master's volume, mute and filters hold the reverb too (#1855): they run on
//! the main track after the reverb bus sums in, so a pause fade or an underwater
//! muffle on Master reaches every tail, not only the dry signal.

use super::loud_hall_tests::hall;
use super::loud_tests::{capture, peak};
use super::*;
use crate::audio::mixer::{EffectChain, Filter, GroupPatch};

/// The capture from 0.5 s on: the gunshot ended at 0.3 s, so only the hall's
/// wet tail is left (stereo is interleaved: 0.5 s is `RATE` samples).
fn tail(samples: &[f32]) -> &[f32] {
    &samples[RATE as usize..]
}

/// One gunshot in the full-wet hall, then `master` applied to Master.
fn shot_in_hall(name: &str, master: GroupPatch) -> Vec<f32> {
    capture(name, 1, 0.5, |maestro| {
        hall(maestro, EffectChain::master());
        maestro.set_group("Master", &master).unwrap();
    })
}

/// How much of a channel's energy is in its sample-to-sample change: high for
/// a bright signal, low for a muffled one.
fn brightness(samples: &[f32]) -> f32 {
    let energy: f32 = samples.iter().map(|s| s * s).sum();
    let change: f32 = samples.windows(3).map(|w| (w[2] - w[0]).powi(2)).sum();
    change / energy.max(f32::MIN_POSITIVE)
}

#[test]
fn master_at_volume_zero_or_muted_silences_the_hall_tail() {
    let open = shot_in_hall("rusty_master_open.wav", GroupPatch::default());
    assert!(peak(tail(&open)) > 0.01, "the hall rings on");
    let zero = GroupPatch {
        volume: Some(0.0),
        ..Default::default()
    };
    let silent = shot_in_hall("rusty_master_zero.wav", zero);
    assert_eq!(peak(tail(&silent)), 0.0, "Master at 0 silences the tail");
    let mute = GroupPatch {
        mute: Some(true),
        ..Default::default()
    };
    let muted = shot_in_hall("rusty_master_mute.wav", mute);
    assert_eq!(peak(tail(&muted)), 0.0, "a muted Master silences the tail");
}

#[test]
fn a_low_pass_on_master_darkens_the_hall_tail() {
    let open = shot_in_hall("rusty_master_bright.wav", GroupPatch::default());
    let muffle = GroupPatch {
        low_pass: Some(Filter::new(300.0, 0.0)),
        ..Default::default()
    };
    let dark = shot_in_hall("rusty_master_dark.wav", muffle);
    let (bright, muffled) = (brightness(tail(&open)), brightness(tail(&dark)));
    assert!(peak(tail(&dark)) > 1e-4, "muffled, not silenced");
    assert!(
        muffled < bright * 0.1,
        "the wet tail is darkened: {muffled} vs {bright}"
    );
}
