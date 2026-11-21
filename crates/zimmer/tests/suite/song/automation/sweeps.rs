//! A filter opening across the piece — the build, read at every moment of every note rather than
//! once at its onset.

use scorsese_zimmer::song::{Param, Song};

use super::setup::{BEATS, at, curve, filtered, frame, render, rms, voiced};
use crate::common::brightness;
use crate::common::songs::voice;

fn sweep() -> scorsese_zimmer::song::Automation {
    curve(Param::Cutoff, vec![at(0.0, 300.0), at(BEATS, 8300.0)])
}

fn struck(beat: f32, cutoff: f32) -> Song {
    let mut song = voiced(filtered(cutoff), 0.5);
    let pattern = song.patterns.get_mut("a").expect("the fixture defines `a`");
    let note = voice(pattern, 0);
    note.start = beat;
    note.dur = 2.0;
    song
}

#[test]
fn a_flat_curve_plays_the_note_written_at_its_value() {
    for (onset, wanted) in [(0.0, 300.0), (4.0, 4300.0)] {
        let mut moving = struck(onset, 300.0);
        moving.automation = vec![curve(
            Param::Cutoff,
            vec![at(0.0, wanted), at(BEATS, wanted)],
        )];
        assert_eq!(
            render(&moving),
            render(&struck(onset, wanted)),
            "a note at beat {onset} under a flat {wanted} Hz is the note written there"
        );
    }
}

#[test]
fn a_held_note_opens_while_it_is_held() {
    let mut song = struck(0.0, 300.0);
    let pattern = song.patterns.get_mut("a").expect("the fixture defines `a`");
    voice(pattern, 0).dur = BEATS;
    song.automation = vec![sweep()];
    let mix = render(&song);
    let (early, late) = (
        brightness(&mix[frame(0.5)..frame(1.5)]),
        brightness(&mix[frame(6.0)..frame(7.0)]),
    );
    assert!(
        late > early * 1.5,
        "the held note stayed shut: {early} → {late}"
    );
}

#[test]
fn two_notes_at_different_beats_are_not_the_same_note() {
    let mut early = struck(0.0, 300.0);
    early.automation = vec![sweep()];
    let mut late = struck(6.0, 300.0);
    late.automation = vec![sweep()];
    let (early, late) = (render(&early), render(&late));
    assert_ne!(
        early[frame(0.5)..frame(2.0)],
        late[frame(6.5)..frame(8.0)],
        "six beats along the sweep is a different instrument"
    );
}

#[test]
fn a_sweep_opens_the_part_note_by_note() {
    let mut song = voiced(filtered(300.0), 0.5);
    let pattern = song.patterns.get_mut("a").expect("the fixture defines `a`");
    voice(pattern, 0).dur = 0.5;
    let first = pattern.notes.clone();
    for beat in 1..BEATS as usize {
        pattern.notes.extend(first.iter().cloned());
        voice(pattern, beat).start = beat as f32;
    }
    song.automation = vec![sweep()];
    let mix = render(&song);
    let bright: Vec<f32> = (0..BEATS as usize)
        .map(|beat| brightness(&mix[frame(beat as f32)..frame(beat as f32 + 0.4)]))
        .collect();
    for pair in bright.windows(2) {
        assert!(
            pair[1] > pair[0],
            "every strike is brighter than the last: {bright:?}"
        );
    }
    assert!(
        rms(&mix) > 0.01,
        "and the part is audible rather than filtered away"
    );
}
