//! What is written is what is played: the arrangement's transforms, chords
//! voiced, swing and marks applied — and what cannot be, said.

use scorsese_zimmer::midi::{Drum, ExportError, export};

use super::read::{read, strikes};
use super::song;

/// Every note-on of the song's tracks, `(tick, channel, key, vel)` per track.
fn played(json: &str, tracks: &[&str]) -> Vec<Vec<(u64, u8, u8, u8)>> {
    let file = read(&export(&song(tracks, json), &[]).expect("exports").bytes);
    file.tracks[1..]
        .iter()
        .map(|track| strikes(track))
        .collect()
}

#[test]
fn the_arrangement_is_played_with_its_transforms_and_chords_voiced() {
    let tracks = played(
        r#""bpm": 120, "arrangement": ["v",
             { "pattern": "v", "transpose": 2, "vel_scale": 2.0, "tracks": ["bass"] }],
           "patterns": { "v": { "beats": 4, "notes": [
             { "track": "bass", "note": "C2", "start": 0, "dur": 1, "vel": 0.5 },
             { "track": "keys", "chord": "C", "oct": 4, "start": 0, "dur": 2 } ] } }"#,
        &["bass", "keys"],
    );
    assert_eq!(tracks[0], [(0, 0, 36, 64), (7_680, 0, 38, 127)]);
    assert_eq!(
        tracks[1],
        [(0, 1, 60, 127), (0, 1, 64, 127), (0, 1, 67, 127)],
        "the chord's voices, and nothing where the entry muted it"
    );
}

#[test]
fn swing_and_marks_are_where_and_how_the_note_is_played() {
    let json = r#""bpm": 120, "swing": 0.5, "arrangement": ["v"],
        "patterns": { "v": { "beats": 2, "notes": [
          { "track": "a", "note": 60, "start": 0.5, "dur": 0.5, "vel": 0.5,
            "articulation": "accent" },
          { "track": "a", "note": 62, "start": 1.0, "dur": 1.0, "articulation": "staccato" }
        ] } }"#;
    let file = read(&export(&song(&["a"], json), &[]).expect("exports").bytes);
    let notes = strikes(&file.tracks[1]);
    assert_eq!(
        notes[0],
        (1_440, 0, 60, 83),
        "swung to ¾ of a beat, struck 1.3×"
    );
    // The staccato note is held half its written beat.
    let release = file.tracks[1]
        .iter()
        .rev()
        .find(|(_, event)| matches!(event, super::read::Event::Off { key: 62, .. }))
        .expect("released");
    assert_eq!(release.0, 1_920 + 960);
}

#[test]
fn what_the_file_cannot_say_is_named() {
    let piece = song(
        &["a"],
        r#""bpm": 120, "humanize": { "timing": 0.01 },
           "fit": { "seconds": 3, "mode": "loop" }, "arrangement": ["v"],
           "patterns": { "v": { "beats": 4, "notes": [
             { "track": "a", "note": 60.5, "start": 0, "dur": 1 },
             { "track": "a", "note": 62, "start": 1, "dur": 1, "articulation": "glide" },
             { "track": "a", "note": 64, "start": 2, "dur": 1, "vel": 0 } ] } }"#,
    );
    let exported = export(&piece, &[]).expect("exports");
    assert_eq!(exported.notes, 2, "the silent note is not written");
    let said = exported.left_out.join("\n");
    for words in [
        "1 notes are between two keys",
        "1 glides are written as plain notes",
        "1 notes play at velocity zero",
        "`humanize` is not written",
        "`fit` is not applied",
    ] {
        assert!(said.contains(words), "no {words:?} in:\n{said}");
    }
}

#[test]
fn a_drum_must_name_a_track_and_a_key_a_key() {
    let piece = song(
        &["kick"],
        r#""bpm": 120, "arrangement": ["v"], "patterns": { "v": { "beats": 1, "notes": [] } }"#,
    );
    let snare: Drum = "snare".parse().expect("reads");
    assert_eq!(
        export(&piece, &[snare]),
        Err(ExportError::NoSuchTrack {
            track: "snare".to_owned()
        })
    );
    let far = Drum {
        track: "kick".to_owned(),
        key: Some(200),
    };
    assert!(matches!(
        export(&piece, &[far]),
        Err(ExportError::DrumKey { .. })
    ));
    for bad in ["kick=200", "kick=x", "=36", " "] {
        assert!(bad.parse::<Drum>().is_err(), "{bad:?} was read");
    }
    let kick: Drum = " kick = 36 ".parse().expect("reads");
    assert_eq!((kick.track.as_str(), kick.key), ("kick", Some(36)));
    assert_eq!(kick.to_string(), "kick=36");

    let broken = song(
        &["kick"],
        r#""bpm": 120, "arrangement": ["gone"], "patterns": {}"#,
    );
    assert!(matches!(export(&broken, &[]), Err(ExportError::Song(_))));
}
