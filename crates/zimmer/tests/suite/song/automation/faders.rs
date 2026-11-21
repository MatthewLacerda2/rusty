//! A fader that moves, read off the samples it produced.
//!
//! Every assertion here is against a **second render** of the same song, so
//! the waveform cancels and what is left is the number the curve had. A test
//! that only asked whether an automated song came out louder somewhere would
//! pass on a curve read at the wrong beat, upside down, or half as fast.

use scorsese_zimmer::song::{Param, Song};

use super::setup::{BEATS, at, curve, frame, held, ramp, render, rms, sides};

/// The fixture with one curve on it.
fn riding(gain: f32, moving: scorsese_zimmer::song::Automation) -> Song {
    let mut song = held(gain);
    song.automation = vec![moving];
    song
}

/// The promise the whole mechanism rests on: a curve parked at the number the
/// document already wrote is that number. Sample for sample, not nearly.
///
/// It is also the strong form of "a song without automation renders what it
/// always did" — an automated track is routed through a bus that a plain one
/// is not, and this says the two arrive at the same `f32`.
#[test]
fn a_flat_curve_renders_exactly_what_the_written_gain_did() {
    let parked = riding(0.5, curve(Param::Gain, vec![at(0.0, 0.5)]));
    assert_eq!(
        render(&held(0.5)),
        render(&parked),
        "riding a fader that does not move is the fader standing still"
    );
}

/// The mechanism, pinned: the level at any beat is the *curve* at that beat.
///
/// Divided out against a render of the same song at a constant fader, so what
/// is compared is the gain and not the waveform.
#[test]
fn the_level_at_a_beat_is_what_the_curve_says_at_that_beat() {
    let flat = render(&riding(0.5, curve(Param::Gain, vec![at(0.0, 0.5)])));
    let moved = render(&riding(0.5, ramp(Param::Gain, 0.0, 0.5)));
    for beat in [0.0, 1.0, 3.0, 5.0, 7.0] {
        let index = frame(beat);
        let expected = flat[index] * (beat / BEATS);
        assert!(
            (moved[index] - expected).abs() < 1e-5,
            "beat {beat}: {} against the {expected} the curve asks for",
            moved[index]
        );
    }
}

/// The build, end to end and in the direction it was written: quiet at the
/// start, loud at the end. The test above could pass on a curve read backwards
/// at every beat but the middle; this one could not.
#[test]
fn a_rising_curve_grows_across_the_piece() {
    let mix = render(&riding(0.6, ramp(Param::Gain, 0.0, 0.6)));
    let quarters: Vec<f32> = (0..4)
        .map(|quarter| rms(&mix[frame(quarter as f32 * 2.0)..frame(quarter as f32 * 2.0 + 2.0)]))
        .collect();
    for pair in quarters.windows(2) {
        assert!(
            pair[1] > pair[0] * 1.3,
            "each quarter has to be above the last: {quarters:?}"
        );
    }
    assert!(
        quarters[3] > quarters[0] * 5.0,
        "and the piece has to arrive somewhere: {quarters:?}"
    );
}

/// A moving `pan` is a moving *position*: the part travels from one side to
/// the other, and the law that keeps it the same loudness on the way is the
/// constant-power law a written `pan` already obeys.
#[test]
fn a_pan_curve_carries_the_part_across_without_changing_its_level() {
    let (left, right) = sides(&riding(0.4, ramp(Param::Pan, -1.0, 1.0)));
    // A short window at each end, because the position is *travelling*: a
    // beat into an eight-beat sweep it is already a quarter of the way across,
    // and the claim being made is about where it starts and stops.
    let opening = frame(0.05);
    let closing = frame(BEATS - 0.05);
    assert!(
        rms(&right[..opening]) < rms(&left[..opening]) * 0.02,
        "it starts hard left"
    );
    assert!(
        rms(&left[closing..]) < rms(&right[closing..]) * 0.02,
        "and ends hard right"
    );
    let centred = render(&held(0.4));
    for beat in [0.5, 2.0, 4.0, 6.0, 7.5] {
        let index = frame(beat);
        let power = left[index].powi(2) + right[index].powi(2);
        let wanted = 2.0 * centred[index].powi(2);
        assert!(
            (power - wanted).abs() < 1e-4,
            "beat {beat}: {power} against {wanted} — the law is constant power"
        );
    }
}

/// A curve on one parameter leaves the other where the document put it, which
/// is what lets a part be automated in level and placed by hand at once.
#[test]
fn automating_the_level_leaves_the_written_position_alone() {
    let mut song = riding(0.5, curve(Param::Gain, vec![at(0.0, 0.5)]));
    song.tracks[0].pan = -1.0;
    let (left, right) = sides(&song);
    assert!(right.iter().all(|sample| sample.abs() < 1e-6), "hard left");
    assert!(left.iter().any(|sample| sample.abs() > 0.1), "and audible");
}

/// And the other way round: a moving position does not quietly take the
/// written fader with it.
#[test]
fn automating_the_position_leaves_the_written_level_alone() {
    let loud = sides(&riding(0.6, curve(Param::Pan, vec![at(0.0, 0.0)])));
    let quiet = sides(&riding(0.3, curve(Param::Pan, vec![at(0.0, 0.0)])));
    let index = frame(2.0);
    assert!(
        (loud.0[index] - quiet.0[index] * 2.0).abs() < 1e-5,
        "half the fader is half the level: {} against {}",
        loud.0[index],
        quiet.0[index]
    );
}
