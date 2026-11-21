//! What becomes a track, and how its notes are written.

use scorsese_zimmer::midi::import;
use scorsese_zimmer::render_song;
use scorsese_zimmer::song::{InlineOnly, Note, PatternEntry, Pitch, Song};

use crate::midi::file::{Track, smf};

/// Every note `track` plays, in arrangement order, as written.
pub(crate) fn notes(song: &Song, track: &str) -> Vec<Note> {
    song.arrangement
        .iter()
        .flat_map(|entry| entry.layers())
        .flat_map(|layer| &song.patterns[layer.pattern()].notes)
        .filter_map(|entry| match entry {
            PatternEntry::Note(note) if note.track == track => Some(note.clone()),
            _ => None,
        })
        .collect()
}

fn names(song: &Song) -> Vec<&str> {
    song.tracks
        .iter()
        .map(|track| track.name.as_str())
        .collect()
}

#[test]
fn each_track_and_channel_is_a_track_and_channel_ten_is_drums() {
    let mut conductor = Track::default();
    conductor.tempo(0, 120);
    let mut piano = Track::default();
    piano
        .name("Piano")
        .note(0, 480, 0, 60)
        .note(480, 240, 0, 64);
    let mut kit = Track::default();
    kit.note(0, 0, 9, 36);
    let song = import(&smf(1, 480, &mut [conductor, piano, kit]))
        .expect("imports")
        .song;

    assert_eq!(
        names(&song),
        ["Piano", "drums"],
        "the conductor plays nothing"
    );
    let played = notes(&song, "Piano");
    assert_eq!(played.len(), 2);
    assert_eq!(played[1].note, Pitch::Name("E4".to_owned()));
    assert_eq!((played[1].start, played[1].dur), (1.0, 0.5));
    assert_eq!(played[1].vel, 0.787, "100/127, to three places");

    let hits = notes(&song, "drums");
    assert_eq!(
        hits[0].note,
        Pitch::Midi(36.0),
        "a drum stays its key number"
    );
    assert_eq!(
        hits[0].dur,
        1.0 / 480.0,
        "released where it started: one tick"
    );
}

#[test]
fn one_midi_track_on_two_channels_is_two_tracks() {
    let mut both = Track::default();
    both.note(0, 480, 0, 48).note(480, 480, 1, 72);
    let song = import(&smf(0, 480, &mut [both])).expect("imports").song;
    assert_eq!(names(&song), ["track-1-ch1", "track-1-ch2"]);
    assert_eq!(
        notes(&song, "track-1-ch2")[0].note,
        Pitch::Name("C5".to_owned())
    );
}

#[test]
fn two_tracks_of_one_name_are_told_apart() {
    let mut first = Track::default();
    first.name("Violin").note(0, 480, 0, 67);
    let mut second = Track::default();
    second.name("Violin").note(0, 480, 1, 64);
    let song = import(&smf(1, 480, &mut [first, second]))
        .expect("imports")
        .song;
    assert_eq!(names(&song), ["Violin", "Violin-2"]);
}

#[test]
fn a_flat_key_is_declared_and_spelled_in_flats() {
    let mut part = Track::default();
    part.key(0, -1, false).note(0, 480, 0, 70);
    let song = import(&smf(0, 96, &mut [part])).expect("imports").song;
    assert_eq!(song.key.as_deref(), Some("F major"));
    assert_eq!(
        notes(&song, "track-1")[0].note,
        Pitch::Name("Bb4".to_owned())
    );
}

#[test]
fn what_comes_in_is_a_valid_song_that_renders_and_round_trips() {
    let mut part = Track::default();
    part.tempo(0, 240).note(0, 96, 0, 60).note(96, 96, 0, 67);
    let mut kit = Track::default();
    kit.note(0, 24, 9, 38);
    let song = import(&smf(1, 96, &mut [part, kit])).expect("imports").song;

    song.validate().expect("valid");
    let samples = render_song(&song, &InlineOnly).expect("renders");
    assert!(
        samples.iter().any(|sample| sample.abs() > 0.01),
        "it makes a sound"
    );
    let json = song.to_json().expect("serialises");
    assert_eq!(Song::from_json(&json).expect("parses"), song);
}
