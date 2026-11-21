//! A track's row when it is summed through a bus of its own — one with a chain,
//! or with a fader that moves — rather than added straight into the master.

use scorsese_zimmer::patch::Fx;
use scorsese_zimmer::song::{Automation, Easing, Param, Point, Song};

use super::tracks::{bake, duet, mean};

/// And the same for a track summed through a bus of its own — one with a
/// chain, or with a fader that moves — whose part is measured where the
/// master hears it, at its fader once and not twice.
#[test]
fn a_bussed_track_is_measured_after_its_gain_too() {
    let mut chained = duet();
    chained.tracks[0].fx = vec![Fx::Saturate {
        drive: 1.0,
        mix: 0.5,
    }];
    let mut quieter = chained.clone();
    quieter.tracks[0].gain = 0.5;
    let moved = mean(&bake(&chained).tracks, "sub") - mean(&bake(&quieter).tracks, "sub");
    assert!(
        (moved - 6.02).abs() < 0.1,
        "a chained part's fader: {moved}"
    );

    let written = mean(&bake(&quieter_sub()).tracks, "sub");
    let mut riding = duet();
    riding.automation = vec![Automation {
        track: "sub".to_owned(),
        param: Param::Gain,
        points: vec![Point {
            beat: 0.0,
            value: 0.5,
            easing: Easing::Linear,
        }],
    }];
    let ridden = mean(&bake(&riding).tracks, "sub");
    assert!(
        (written - ridden).abs() < 0.01,
        "{ridden} against {written}"
    );
}

/// [`duet`] with its bass at half its fader.
fn quieter_sub() -> Song {
    let mut song = duet();
    song.tracks[0].gain = 0.5;
    song
}
