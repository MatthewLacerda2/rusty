//! The key and the degree as they sit in the saved document.

use super::setup::{degree, playing, triad};
use crate::common::songs::verse;
use rusty_zimmer::song::{Degree, DegreeNote, PatternEntry};
use rusty_zimmer::{Song, SynthError};

#[test]
fn a_key_and_a_degree_round_trip_as_they_were_written() {
    let written = playing(
        Some("Db minor"),
        vec![degree(Degree::Altered("b3".to_owned()), 3, 0.0)],
    );
    let json = written.to_json().expect("the song serialises");
    assert!(json.contains(r#""key": "Db minor""#), "{json}");
    assert_eq!(Song::from_json(&json).expect("it parses back"), written);
    let parsed = Song::from_json(&json).expect("it parses back");
    assert!(matches!(
        parsed.patterns["verse"].notes[0],
        PatternEntry::Degree(_)
    ));

    // Each of the other three fields beside `degree`, which is the pairing the fourth variant made
    // newly possible and the one declaration order would otherwise have picked a winner for.
    for second in [
        r#""note": "C4""#,
        r#""chord": "Dm7""#,
        r#""steps": "x-x-", "div": 0.5"#,
    ] {
        let two_kinds = json.replace(r#""degree": "b3""#, &format!(r#""degree": 3, {second}"#));
        assert!(
            Song::from_json(&two_kinds).is_err(),
            "an entry that is a degree and something else must be refused: {two_kinds}"
        );
    }
}

#[test]
fn a_degree_is_checked_the_way_every_other_entry_is() {
    let of = |written: DegreeNote| {
        let mut song = playing(Some("D minor"), triad(["D4", "E4", "F4"]));
        verse(&mut song).notes.push(written.into());
        song.validate().expect_err("the song is refused")
    };
    let fourth = |edit: fn(&mut DegreeNote)| {
        let mut written = DegreeNote {
            track: "bass".to_owned(),
            degree: Degree::Plain(1),
            oct: Some(4),
            start: 0.0,
            dur: 0.5,
            vel: 1.0,
            articulation: None,
        };
        edit(&mut written);
        written
    };
    assert_eq!(
        of(fourth(|it| it.track = "nobody".to_owned())),
        SynthError::UnknownTrack {
            pattern: "verse".to_owned(),
            index: 3,
            track: "nobody".to_owned(),
        }
    );
    assert_eq!(
        of(fourth(|it| it.start = -0.5)),
        SynthError::BadNoteStart {
            pattern: "verse".to_owned(),
            index: 3,
            start: -0.5,
        }
    );
    assert_eq!(
        of(fourth(|it| it.dur = 0.0)),
        SynthError::BadNoteDuration {
            pattern: "verse".to_owned(),
            index: 3,
            dur: 0.0,
        }
    );
}
