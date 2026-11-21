//! What a curve does when `fit` decides the length.
//!
//! Beats are counted from the start of the **rendered piece**, once. That is
//! one decision with two visible consequences, and both are asserted here: a
//! `loop` does not take the curve back to the beginning with it, and a
//! `stretch` carries the curve along with the tempo it moved.

use scorsese_zimmer::song::{Fit, FitMode, Param};
use scorsese_zimmer::{SAMPLE_RATE, song::Song};

use super::setup::{BEATS, at, curve, frame, held, render, rms};

/// The fixture at a target length, with a gain curve over `beats` of it.
fn fitted(seconds: f32, mode: FitMode, beats: f32, to: f32) -> Song {
    let mut song = held(0.5);
    song.fit = Some(Fit::lasting(seconds, mode));
    song.automation = vec![curve(Param::Gain, vec![at(0.0, 0.0), at(beats, to)])];
    song
}

/// A build across a looped bed builds **once**. A curve that went back with
/// the arrangement would be a saw — the same swell over and over — which is
/// the defect this decision exists to avoid.
///
/// The fixture is four seconds, so eight is two passes; the curve spans both.
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

/// The same song without a curve loops to two passes of one level, which is
/// what the assertion above is measured against: the difference is the curve
/// and not the loop.
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

/// `stretch` moves the tempo and leaves the beats alone, so a build over eight
/// bars is still a build over eight bars — read at the tempo the piece is
/// actually played at, not the one it was written at.
///
/// The fixture is eight beats of 120 bpm — four seconds — stretched to five,
/// which is 96 bpm. At four seconds in, that is beat 6.4 and not beat 8, so a
/// curve read at the written tempo would already have finished.
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
