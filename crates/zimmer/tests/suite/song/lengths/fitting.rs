//! `fit`: landing on a target length, and refusing when it cannot.

use super::setup::{fitted, render, samples};
use crate::common::peak;
use scorsese_zimmer::SynthError;
use scorsese_zimmer::song::FitMode;

#[test]
fn looping_lands_on_the_target_to_the_sample() {
    for seconds in [0.7, 2.0, 5.0, 43.0] {
        let rendered = render(&fitted(seconds, FitMode::Loop));
        assert_eq!(
            rendered.len(),
            samples(seconds),
            "a {seconds}s loop must be exactly that long"
        );
    }
}

/// A loop is a bed: a gap in the middle of one is worse than a seam, so the
/// music has to actually reach the end rather than being padded to it.
#[test]
fn a_loop_is_music_all_the_way_to_the_target() {
    let rendered = render(&fitted(7.0, FitMode::Loop));
    let last_second = &rendered[samples(5.5)..samples(6.5)];
    assert!(
        peak(last_second) > 0.01,
        "the loop went quiet before the target"
    );
}

/// Truncation lands mid-waveform, which is a click. The seam fade is what
/// makes an arbitrary cut usable, so the final sample has to be at rest.
#[test]
fn a_cut_loop_does_not_end_on_a_click() {
    let rendered = render(&fitted(3.3, FitMode::Loop));
    let last = rendered.last().copied().expect("samples");
    assert!(
        last.abs() < 1e-3,
        "the cut ends at {last}, which is a click"
    );
}

#[test]
fn once_plays_through_and_pads_the_rest_with_silence() {
    let rendered = render(&fitted(6.0, FitMode::Once));
    assert_eq!(rendered.len(), samples(6.0));
    // The fixture is two seconds of music; everything past it is padding.
    assert!(
        peak(&rendered[samples(3.0)..]) < 1e-6,
        "a sting must not be looped into a texture"
    );
}

/// `stretch` keeps the music intact and moves the tempo instead, so it lands
/// on a whole number of passes and on the target exactly.
///
/// 4.4 s of a 2 s song is two passes at 109.09 bpm rather than 120, which puts
/// the last note on beat 7 at 3.85 s — where at the written tempo it would
/// have fallen at 3.50 s and the buffer would have ended at 4.00 s.
#[test]
fn stretching_moves_the_tempo_to_land_on_the_target() {
    let rendered = render(&fitted(4.4, FitMode::Stretch));
    assert_eq!(rendered.len(), samples(4.4));
    assert!(
        peak(&rendered[samples(3.85)..samples(3.95)]) > 0.001,
        "the last note did not move with the tempo"
    );
    assert!(
        peak(&rendered[samples(3.6)..samples(3.8)]) < 0.001,
        "and it is no longer where the written tempo would have put it"
    );
}

/// The refusal that keeps `stretch` honest: a bed at 40% speed is not a bed.
#[test]
fn stretching_too_far_is_refused_with_the_tempo_it_would_have_needed() {
    // 3 s of a 2 s song is two passes crammed into three seconds: 160 bpm
    // against the 120 written, a third faster, which is past what a piece of
    // music survives.
    let refused = fitted(3.0, FitMode::Stretch)
        .validate()
        .expect_err("a third faster is too far");
    let SynthError::StretchTooFar { bpm, needed, .. } = refused else {
        panic!("wrong refusal: {refused}");
    };
    assert_eq!(bpm, 120.0);
    assert!(
        (needed - 160.0).abs() < 0.1,
        "it should name the tempo it needed, got {needed}"
    );
    // And the same target under `loop` is fine, which is what the message
    // tells the caller to reach for.
    assert_eq!(fitted(3.0, FitMode::Loop).validate(), Ok(()));
}

#[test]
fn a_target_that_is_not_a_length_is_refused() {
    for seconds in [0.0, -3.0] {
        assert_eq!(
            fitted(seconds, FitMode::Loop).validate(),
            Err(SynthError::BadFitSeconds { seconds })
        );
    }
}
