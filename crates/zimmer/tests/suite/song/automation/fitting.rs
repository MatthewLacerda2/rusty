//! What a curve does when `fit` decides the length.

use rusty_zimmer::song::{Fit, FitMode, Param};
use rusty_zimmer::{song::Song, SAMPLE_RATE};

use super::setup::{at, curve, frame, held, render, rms, BEATS};

fn fitted(seconds: f32, mode: FitMode, beats: f32, to: f32) -> Song {
    let mut song = held(0.5);
    song.fit = Some(Fit::lasting(seconds, mode));
    song.automation = vec![curve(Param::Gain, vec![at(0.0, 0.0), at(beats, to)])];
    song
}

/// The fixture is four seconds, so eight is two passes, and the curve spans both.
#[test]
fn a_curve_does_not_go_back_when_the_arrangement_loops() {
    let mix = render(&fitted(8.0, FitMode::Loop, BEATS * 2.0, 0.5));
    let (first, second) = mix.split_at(frame(BEATS));
    let (quiet, loud) = (rms(first), rms(second));
    assert!(
        loud > quiet * 2.0,
        "the second pass is further up the build, not back at the bottom: \
         {quiet} then {loud}"
    );
}

#[test]
fn a_looped_bed_with_no_curve_is_the_same_level_both_passes() {
    let mut song = held(0.5);
    song.fit = Some(Fit::lasting(8.0, FitMode::Loop));
    let mix = render(&song);
    let (first, second) = mix.split_at(frame(BEATS));
    let (before, after) = (rms(first), rms(second));
    assert!(
        (before - after).abs() < before * 0.05,
        "{before} against {after}"
    );
}

/// Four seconds of 120 bpm stretched to five is 96 bpm: four seconds in is beat 6.4, not 8.
#[test]
fn a_stretch_fit_carries_the_curve_along_with_the_tempo() {
    let flat = {
        let mut song = held(0.5);
        song.fit = Some(Fit::lasting(5.0, FitMode::Stretch));
        song.automation = vec![curve(Param::Gain, vec![at(0.0, 0.5)])];
        render(&song)
    };
    let moved = render(&fitted(5.0, FitMode::Stretch, BEATS, 0.5));
    let stretched_beat = 96.0 / 60.0;
    for seconds in [1.0, 2.5, 4.0] {
        let index = (seconds * SAMPLE_RATE as f32).round() as usize;
        let expected = flat[index] * (seconds * stretched_beat / BEATS);
        assert!(
            (moved[index] - expected).abs() < 1e-5,
            "{seconds}s in: {} against the {expected} the stretched curve asks for",
            moved[index]
        );
    }
}
