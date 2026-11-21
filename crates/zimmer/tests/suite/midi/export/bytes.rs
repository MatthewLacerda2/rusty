//! What an exported file is, byte for byte where it is small enough to say.

use scorsese_zimmer::midi::{Drum, TICKS_PER_BEAT, export};

use super::read::{Event, read, strikes};
use super::song;

/// One note, one track, one tempo — written out by hand from the
/// specification, so the encoder is checked against the format rather than
/// against itself.
#[test]
fn the_smallest_song_is_exactly_these_bytes() {
    let one = song(
        &["lead"],
        r#""bpm": 120, "arrangement": ["a"], "patterns": { "a": { "beats": 1,
           "notes": [{ "track": "lead", "note": "C4", "start": 0, "dur": 1 }] } }"#,
    );
    let expected: &[u8] = &[
        // The header: six bytes of format 1, two tracks, 1920 ticks a beat.
        b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 1, 0, 2, 0x07, 0x80, //
        b'M', b'T', b'r', b'k', 0, 0, 0, 11, //
        0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, // 500 000 µs a beat: 120 bpm
        0x00, 0xFF, 0x2F, 0x00, //
        b'M', b'T', b'r', b'k', 0, 0, 0, 21, //
        0x00, 0xFF, 0x03, 0x04, b'l', b'e', b'a', b'd', // its name
        0x00, 0x90, 60, 127, // C4 at full velocity, channel 1
        0x8F, 0x00, 0x80, 60, 0, // released 1920 ticks later
        0x00, 0xFF, 0x2F, 0x00,
    ];
    assert_eq!(export(&one, &[]).expect("exports").bytes, expected);
}

#[test]
fn a_major_or_minor_key_is_a_signature_and_a_mode_is_not() {
    let keyed = |key: &str| {
        let piece = song(
            &["lead"],
            &format!(
                r#""bpm": 90, "key": "{key}", "arrangement": ["a"], "patterns": {{ "a": {{
                   "beats": 1, "notes": [{{ "track": "lead", "note": 60, "start": 0, "dur": 1 }}] }} }}"#
            ),
        );
        let exported = export(&piece, &[]).expect("exports");
        let conductor = read(&exported.bytes).tracks.remove(0);
        let signature = conductor.iter().find_map(|(_, event)| match event {
            Event::Key(sharps, minor) => Some((*sharps, *minor)),
            _ => None,
        });
        (signature, exported.left_out)
    };
    assert_eq!(keyed("Eb major").0, Some((-3, false)));
    assert_eq!(keyed("F# minor").0, Some((3, true)));
    assert_eq!(
        keyed("D# major").0,
        Some((-3, false)),
        "as Eb, not nine sharps"
    );
    let (none, said) = keyed("D dorian");
    assert_eq!(none, None);
    assert!(
        said.iter().any(|line| line.contains("D dorian")),
        "{said:?}"
    );
}

#[test]
fn every_track_has_its_own_channel_past_the_drums_and_drums_are_channel_ten() {
    let names: Vec<String> = (1..=11).map(|n| format!("t{n}")).collect();
    let tracks: Vec<&str> = names.iter().map(String::as_str).collect();
    let notes: Vec<String> = names
        .iter()
        .map(|name| format!(r#"{{ "track": "{name}", "note": "C2", "start": 0, "dur": 1 }}"#))
        .collect();
    let piece = song(
        &tracks,
        &format!(
            r#""bpm": 120, "arrangement": ["a"],
               "patterns": {{ "a": {{ "beats": 1, "notes": [{}] }} }}"#,
            notes.join(",")
        ),
    );
    let kick: Drum = "t3=36".parse().expect("reads");
    let file = read(&export(&piece, &[kick]).expect("exports").bytes);
    assert_eq!((file.format, file.ppq), (1, TICKS_PER_BEAT));
    let channels: Vec<u8> = file.tracks[1..]
        .iter()
        .map(|track| strikes(track)[0].1)
        .collect();
    assert_eq!(channels, [0, 1, 9, 2, 3, 4, 5, 6, 7, 8, 10]);
    assert_eq!(
        strikes(&file.tracks[3])[0].2,
        36,
        "every hit on the key given"
    );
    assert_eq!(file.tracks[1][0].1, Event::Name("t1".to_owned()));
}
