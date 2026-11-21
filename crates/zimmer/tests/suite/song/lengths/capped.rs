//! The longest song a recipe may ask to be rendered: ten minutes, refused by
//! `validate` before a sample buffer is allocated. Every song here is checked,
//! never rendered: a song at the cap is ten minutes of samples.

use scorsese_zimmer::Song;
use scorsese_zimmer::song::{Fit, FitMode};

use crate::common::songs::{note, played, song};

/// The fixture at 60 bpm, its one pattern `beats` long: a beat a second.
fn lasting(beats: f32) -> Song {
    let mut song = Song {
        bpm: 60.0,
        ..song()
    };
    let verse = song
        .patterns
        .get_mut("verse")
        .expect("the fixture's pattern");
    verse.beats = beats;
    song.arrangement.truncate(1);
    song
}

#[test]
fn a_song_one_beat_past_ten_minutes_is_refused_and_one_at_them_is_not() {
    assert_eq!(lasting(600.0).validate(), Ok(()));
    let refused = lasting(601.0).validate().expect_err("past the cap");
    let said = refused.to_string();
    assert!(said.contains("601") && said.contains("600"), "{said}");
}

#[test]
fn every_slot_of_the_arrangement_counts() {
    let twice = |beats: f32| {
        let mut song = lasting(beats);
        song.arrangement.push("verse".into());
        song.validate()
    };
    assert_eq!(twice(300.0), Ok(()));
    assert!(twice(300.5).is_err(), "601 s over two slots");
}

#[test]
fn a_loop_fit_cannot_ask_for_more_than_ten_minutes() {
    let mut fine = song();
    fine.fit = Some(Fit::lasting(600.0, FitMode::Loop));
    assert_eq!(fine.validate(), Ok(()));
    let mut long = song();
    long.fit = Some(Fit::lasting(1.0e6, FitMode::Loop));
    assert!(long.validate().is_err(), "a million seconds of loop");
}

#[test]
fn a_loop_is_measured_by_the_whole_passes_it_renders_before_its_cut() {
    // Seven seconds a pass: 86 passes render 602 s before the cut to 600.
    let mut song = lasting(7.0);
    song.fit = Some(Fit::lasting(600.0, FitMode::Loop));
    let said = song.validate().expect_err("602 s rendered").to_string();
    assert!(said.contains("602"), "{said}");
    // 85 passes are 595 s, and a fit inside them is fine.
    song.fit = Some(Fit::lasting(595.0, FitMode::Loop));
    assert_eq!(song.validate(), Ok(()));
}

#[test]
fn a_loop_counts_the_last_pass_as_far_as_its_last_note_rings() {
    // 85 passes of 7 s are 595 s, and the last note holds 6 s past its pass.
    let mut song = lasting(7.0);
    let verse = song
        .patterns
        .get_mut("verse")
        .expect("the fixture's pattern");
    verse.notes = played(vec![note("bass", "E2", 6.5, 6.0)]);
    song.fit = Some(Fit::lasting(595.0, FitMode::Loop));
    assert!(song.validate().is_err(), "601 s rendered");
}

#[test]
fn a_note_placed_far_past_its_pattern_counts_toward_the_length() {
    let mut song = lasting(4.0);
    let verse = song
        .patterns
        .get_mut("verse")
        .expect("the fixture's pattern");
    verse.notes = played(vec![note("bass", "E2", 1.0e6, 0.5)]);
    assert!(song.validate().is_err(), "a note a million beats in");

    // Ten minutes of pattern, and a note held a quarter beat past them.
    let mut song = lasting(600.0);
    let verse = song
        .patterns
        .get_mut("verse")
        .expect("the fixture's pattern");
    verse.notes = played(vec![note("bass", "E2", 599.75, 0.5)]);
    assert!(song.validate().is_err(), "a note ringing past the cap");
}

#[test]
fn a_note_ringing_past_a_later_slot_counts_from_where_that_slot_starts() {
    // Two 300-beat slots; the verse's last note holds a beat past its end.
    let mut song = lasting(300.0);
    song.arrangement.push("verse".into());
    let verse = song
        .patterns
        .get_mut("verse")
        .expect("the fixture's pattern");
    verse.notes = played(vec![note("bass", "E2", 299.5, 1.5)]);
    let said = song.validate().expect_err("601 s rendered").to_string();
    assert!(said.contains("601"), "{said}");
}
