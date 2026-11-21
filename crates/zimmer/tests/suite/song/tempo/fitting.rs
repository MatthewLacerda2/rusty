//! A tempo map under `fit`: each pass plays the map again, and a stretch moves
//! every tempo in it by one factor.

use scorsese_zimmer::SynthError;
use scorsese_zimmer::song::{Fit, FitMode, Song};

use super::setup::{close, ends, slowing};

fn fitted(seconds: f32, mode: FitMode) -> Song {
    Song {
        fit: Some(Fit::lasting(seconds, mode)),
        ..slowing()
    }
}

#[test]
fn a_loop_plays_the_map_again_every_pass() {
    // One pass is three seconds, so seven takes three passes, and every one
    // of them slows on its own beat 2.
    let song = fitted(7.0, FitMode::Loop);
    assert!(close(&ends(&song), &[1.0, 3.0, 4.0, 6.0, 7.0, 9.0]));
}

#[test]
fn a_stretch_moves_the_whole_map_by_one_factor() {
    // Three seconds stretched to 3.3: every tempo slows by the same tenth, so
    // the jump still falls on beat 2 and the pass lands on the target.
    let song = fitted(3.3, FitMode::Stretch);
    assert!(close(&ends(&song), &[1.1, 3.3]), "{:?}", ends(&song));
}

#[test]
fn a_stretch_too_far_is_refused_against_the_opening_tempo() {
    let refusal = fitted(4.4, FitMode::Stretch)
        .validate()
        .expect_err("a third slower is past what a piece survives");
    let SynthError::StretchTooFar { bpm, needed, .. } = refusal else {
        panic!("refused for the wrong reason: {refusal:?}")
    };
    assert_eq!(bpm, 120.0);
    assert!((needed - 120.0 * 3.0 / 4.4).abs() < 0.01, "{needed}");
}
