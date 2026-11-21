//! A step string as one entry, held to the notes it replaced.

use crate::common::songs::{blip, note, played, song, verse};
use scorsese_zimmer::song::{Humanize, InlineOnly, Note, PatchRef, PatternEntry, Pitch, Steps};
use scorsese_zimmer::{Song, SynthError, render_song};

const PLAIN: f32 = 0.4;

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

fn steps(pattern: &str) -> Steps {
    Steps {
        track: "bass".to_owned(),
        steps: pattern.to_owned(),
        div: 0.5,
        start: 0.0,
        dur: None,
        note: None,
        vel: PLAIN,
        articulation: None,
    }
}

fn hit(start: f32, vel: f32) -> Note {
    Note {
        track: "bass".to_owned(),
        note: Pitch::Midi(60.0),
        start,
        dur: 0.5,
        vel,
        articulation: None,
    }
}

fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut one_pass = song();
    one_pass.arrangement = vec!["verse".into()];
    one_pass.tracks[0].patch = PatchRef::Inline(Box::new(blip()));
    verse(&mut one_pass).notes = entries;
    one_pass
}

fn stepped() -> Song {
    playing(vec![
        steps("x-xX").into(),
        note("bass", "E2", 1.75, 0.25).into(),
    ])
}

fn spelled() -> Song {
    let mut hits = played(vec![hit(0.0, PLAIN), hit(1.0, PLAIN), hit(1.5, 1.0)]);
    hits.push(note("bass", "E2", 1.75, 0.25).into());
    playing(hits)
}

#[test]
fn a_step_string_equals_writing_its_hits_out_by_hand() {
    assert_eq!(render(&stepped()), render(&spelled()));
}

#[test]
fn a_step_string_swings_and_scatters_as_written_notes_do() {
    let performed = |song: Song| {
        render(&Song {
            swing: 0.4,
            humanize: Some(Humanize {
                timing: 0.02,
                velocity: 0.3,
                timbre: 0.2,
            }),
            ..song
        })
    };
    let played = performed(stepped());
    assert_eq!(played, performed(spelled()));
    assert_ne!(
        played,
        render(&stepped()),
        "the performance did nothing to a step string"
    );
}

#[test]
fn a_character_that_is_not_a_step_is_refused_by_name() {
    let typo = playing(vec![steps("x-x?").into()]);
    assert!(
        matches!(
            typo.validate(),
            Err(SynthError::BadStep { character, step, .. }) if character == '?' && step == 3
        ),
        "an unrecognised character must be refused, and located"
    );
}

#[test]
fn a_string_that_does_not_fill_its_pattern_is_refused() {
    let short = playing(vec![steps("x-x").into()]);
    assert!(matches!(
        short.validate(),
        Err(SynthError::StepsDoNotFit {
            written: 3,
            needed: 4,
            ..
        })
    ));
}

#[test]
fn a_step_entry_is_told_apart_by_its_fields_and_survives_a_round_trip() {
    let written = stepped();
    let reloaded = Song::from_json(&written.to_json().expect("serialise")).expect("deserialise");
    assert_eq!(reloaded, written);
    assert!(
        matches!(reloaded.patterns["verse"].notes[0], PatternEntry::Steps(_)),
        "a step entry came back as something else"
    );

    for confused in [
        r#"{ "track": "b", "chord": "Dm7", "steps": "x-xX", "div": 0.5, "start": 0, "dur": 1 }"#,
        // A misspelled field, which would otherwise be a velocity silently at 1.
        r#"{ "track": "b", "steps": "x-xX", "div": 0.5, "vell": 0.4 }"#,
        r#"{ "track": "b", "steps": "x-xX" }"#,
    ] {
        assert!(
            serde_json::from_str::<PatternEntry>(confused).is_err(),
            "`{confused}` should not have parsed"
        );
    }
}

#[test]
fn a_step_entry_writes_down_its_start_only_when_it_has_one() {
    let json = playing(vec![steps("x-xX").into()])
        .to_json()
        .expect("serialise");
    assert!(
        !json.contains("\"start\""),
        "a string starting at the top of its pattern wrote a `start` it did not need:\n{json}"
    );

    let late = playing(vec![
        Steps {
            start: 1.0,
            ..steps("xX")
        }
        .into(),
    ]);
    let reloaded = Song::from_json(&late.to_json().expect("serialise")).expect("deserialise");
    assert_eq!(reloaded, late, "a `start` that was not the top was lost");
}

#[test]
fn a_step_string_names_a_real_track() {
    let elsewhere = playing(vec![
        Steps {
            track: "hat".to_owned(),
            ..steps("x-xX")
        }
        .into(),
    ]);
    assert!(matches!(
        elsewhere.validate(),
        Err(SynthError::UnknownTrack { track, .. }) if track == "hat"
    ));
}
