//! What is refused, and what is said about what the song cannot hold.

use scorsese_zimmer::midi::{MidiError, import};

use crate::midi::file::{Track, smf};

fn one_note() -> Track {
    let mut track = Track::default();
    track.note(0, 480, 0, 60);
    track
}

#[test]
fn bytes_that_are_not_midi_are_refused_in_words() {
    let refused = import(b"RIFF....WAVEfmt ").expect_err("not midi");
    assert!(
        matches!(refused, MidiError::Unreadable { .. }),
        "{refused:?}"
    );
    assert!(refused.to_string().contains("not a readable MIDI file"));
}

#[test]
fn a_file_timed_in_frames_is_refused() {
    // 25 frames a second, 40 ticks a frame: the high byte is -25.
    let refused = import(&smf(1, 0xE728, &mut [one_note()])).expect_err("smpte");
    assert_eq!(refused, MidiError::Timecode);
}

#[test]
fn a_format_two_file_is_refused() {
    let refused = import(&smf(2, 480, &mut [one_note()])).expect_err("sequential");
    assert_eq!(refused, MidiError::Sequential);
}

#[test]
fn a_file_with_no_notes_is_refused() {
    let mut silent = Track::default();
    silent.tempo(0, 100).program(0, 0, 5);
    assert_eq!(import(&smf(1, 480, &mut [silent])), Err(MidiError::NoNotes));
}

#[test]
fn a_clean_file_leaves_nothing_out() {
    let imported = import(&smf(0, 480, &mut [one_note()])).expect("imports");
    assert!(imported.left_out.is_empty(), "{:?}", imported.left_out);
}

#[test]
fn what_the_song_cannot_hold_is_counted_and_named() {
    let mut piano = Track::default();
    piano
        .name("piano")
        .program(0, 0, 0)
        .pedal(0, 0, 127)
        .on(0, 0, 60, 90)
        .pedal(480, 0, 0)
        .on(480, 0, 64, 90)
        .off(960, 0, 64);
    let said = import(&smf(1, 480, &mut [piano]))
        .expect("imports")
        .left_out
        .join("\n");
    assert!(
        said.contains("2 controller changes (sustain pedal"),
        "{said}"
    );
    assert!(said.contains("`piano` program 1"), "{said}");
    assert!(said.contains("1 notes were never released"), "{said}");
}
