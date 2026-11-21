//! Where a glide comes from, read off the waveform rather than off the field.

use super::setup::{note, playing, rendered};
use crate::common::{measured_hz, peak};
use rusty_zimmer::song::Articulation::Glide;
use rusty_zimmer::song::{ArrangementEntry, Pattern, PatternEntry, Play};
use rusty_zimmer::SAMPLE_RATE;

const ONSET: usize = SAMPLE_RATE as usize;

fn hz(buf: &[f32], onset: usize, from: f32, long: f32) -> f32 {
    let at = |ms: f32| onset + (ms * SAMPLE_RATE as f32 / 1_000.0) as usize;
    measured_hz(&buf[at(from)..at(from + long)], SAMPLE_RATE as f32)
}

fn after(previous: Option<&str>) -> Vec<f32> {
    let mut entries = vec![note("E4", 2.0, 1.0, 0.5, previous.map(|_| Glide)).into()];
    if let Some(name) = previous {
        entries.push(note(name, 0.0, 1.0, 0.5, None).into());
    }
    rendered(&playing(entries))
}

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

#[test]
fn a_glide_with_nothing_before_it_is_the_plain_note() {
    let alone = |mark| rendered(&playing(vec![note("E4", 2.0, 1.0, 0.5, mark).into()]));
    assert_eq!(alone(Some(Glide)), alone(None));
}

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

fn silenced() -> ArrangementEntry {
    ArrangementEntry::from(Play {
        pattern: "lead".to_owned(),
        transpose: None,
        transpose_degrees: None,
        vel_scale: None,
        tracks: Some(vec![]),
    })
}
