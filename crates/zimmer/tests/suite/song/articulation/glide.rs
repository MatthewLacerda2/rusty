//! Where a glide comes from, read off the waveform rather than off the field.
//!
//! The mark is the only one whose effect is a *pitch*, so every assertion here
//! is a frequency — and every one of them is signed. A test that a slid note's
//! pitch moved would pass for a slide in the wrong direction, which is the one
//! mistake this mark can make that still sounds like music.

use super::setup::{note, playing, rendered};
use crate::common::{measured_hz, peak};
use scorsese_zimmer::SAMPLE_RATE;
use scorsese_zimmer::song::Articulation::Glide;
use scorsese_zimmer::song::{ArrangementEntry, Pattern, PatternEntry, Play};

/// Where the note under test starts when the fixture plays one pattern: two
/// beats in, at the fixture's 120 bpm.
const ONSET: usize = SAMPLE_RATE as usize;

/// The pitch of `buf`, over a window `from` milliseconds after `onset` and
/// `long` milliseconds wide.
///
/// A window rather than the whole note because a slide is *going somewhere*:
/// what it reads is the average pitch over the window, which is the number the
/// assertions below are written against.
fn hz(buf: &[f32], onset: usize, from: f32, long: f32) -> f32 {
    let at = |ms: f32| onset + (ms * SAMPLE_RATE as f32 / 1_000.0) as usize;
    measured_hz(&buf[at(from)..at(from + long)], SAMPLE_RATE as f32)
}

/// E4 two beats in, slid onto from `previous` — or struck plain, when there is
/// no previous note to write.
fn after(previous: Option<&str>) -> Vec<f32> {
    let mut entries = vec![note("E4", 2.0, 1.0, 0.5, previous.map(|_| Glide)).into()];
    if let Some(name) = previous {
        entries.push(note(name, 0.0, 1.0, 0.5, None).into());
    }
    rendered(&playing(entries))
}

/// A glide starts where the hand was, and **which way** is the whole of it:
/// from the note above it comes down, from the note below it comes up.
#[test]
fn a_glide_slides_from_the_previous_pitch_in_the_direction_that_pitch_lies() {
    let written = hz(&after(None), ONSET, 2.0, 25.0);
    let from_above = hz(&after(Some("E5")), ONSET, 2.0, 25.0);
    let from_below = hz(&after(Some("E3")), ONSET, 2.0, 25.0);
    assert!(
        from_above > written * 1.3,
        "an octave above the note did not start it high: {from_above} against {written}"
    );
    assert!(
        from_below < written * 0.77,
        "an octave below the note did not start it low: {from_below} against {written}"
    );
}

/// And it arrives. A note whose pitch is still on its way when anybody is
/// listening to it is out of tune rather than expressive.
#[test]
fn a_glide_is_on_the_written_pitch_long_before_the_note_ends() {
    let written = hz(&after(None), ONSET, 100.0, 100.0);
    for previous in ["E5", "E3"] {
        let slid = hz(&after(Some(previous)), ONSET, 100.0, 100.0);
        assert!(
            (slid / written - 1.0).abs() < 0.02,
            "still sliding from {previous} at 100 ms: {slid} against {written}"
        );
    }
}

/// The first note of a track has nothing to slide from, and is the note as
/// written — sample for sample, since a glide of nowhere is not a glide.
#[test]
fn a_glide_with_nothing_before_it_is_the_plain_note() {
    let alone = |mark| rendered(&playing(vec![note("E4", 2.0, 1.0, 0.5, mark).into()]));
    assert_eq!(alone(Some(Glide)), alone(None));
}

/// A part where *every* note is slid onto still slides. The mark is on each
/// note rather than on the piece, so a line of nothing but glides is the
/// ordinary case for a bassline and not a special one.
#[test]
fn a_line_of_nothing_but_glides_still_slides() {
    let all_slid = rendered(&playing(vec![
        note("E5", 0.0, 1.0, 0.5, Some(Glide)).into(),
        note("E4", 2.0, 1.0, 0.5, Some(Glide)).into(),
    ]));
    let written = hz(&after(None), ONSET, 2.0, 25.0);
    let slid = hz(&all_slid, ONSET, 2.0, 25.0);
    assert!(
        slid > written * 1.3,
        "a line of glides did not slide: {slid} against {written}"
    );
}

/// A note the arrangement silenced is still where the hand was. The first pass
/// below plays nothing at all, and the note opening the second pass slides
/// from it anyway — which is what keeps muting eight bars from changing how
/// the ninth is played.
#[test]
fn a_glide_slides_from_a_note_that_never_sounded() {
    let second_pass = 2 * ONSET;
    let opening = |lead: Vec<PatternEntry>| {
        let mut song = playing(vec![note("E4", 0.0, 1.0, 0.5, Some(Glide)).into()]);
        song.patterns.insert(
            "lead".to_owned(),
            Pattern {
                beats: 4.0,
                notes: lead,
            },
        );
        song.arrangement = vec![silenced(), "verse".into()];
        rendered(&song)
    };

    let after_a_silenced_e5 = opening(vec![note("E5", 0.0, 1.0, 0.5, None).into()]);
    assert_eq!(
        peak(&after_a_silenced_e5[..second_pass]),
        0.0,
        "the first pass was supposed to be silent"
    );
    let slid = hz(&after_a_silenced_e5, second_pass, 2.0, 25.0);
    let written = hz(&opening(vec![]), second_pass, 2.0, 25.0);
    assert!(
        slid > written * 1.3,
        "the silenced note was not where the hand was: {slid} against {written}"
    );
}

/// The `lead` pattern, played with every track silenced.
fn silenced() -> ArrangementEntry {
    ArrangementEntry::from(Play {
        pattern: "lead".to_owned(),
        transpose: None,
        transpose_degrees: None,
        vel_scale: None,
        tracks: Some(vec![]),
    })
}
