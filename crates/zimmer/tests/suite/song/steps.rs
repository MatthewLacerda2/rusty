//! A step string as one entry, held to the notes it replaced.
//!
//! The claim the notation lives or dies by: **writing a rhythm as a string must
//! equal writing its hits out by hand** — the same onsets, the same velocities,
//! the same ordinals, and therefore the same performance on top of them. If it
//! does not, the shorthand has stopped meaning the long form and a document
//! that reads as `x-xX` is playing something else.

use crate::common::songs::{blip, note, played, song, verse};
use scorsese_zimmer::song::{Humanize, InlineOnly, Note, PatchRef, PatternEntry, Pitch, Steps};
use scorsese_zimmer::{Song, SynthError, render_song};

/// Velocity of a plain `x` in these fixtures — below 1, so the accents have
/// somewhere to be.
const PLAIN: f32 = 0.4;

/// Renders a song whose instruments are all inline.
fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// A step string over the fixture's two-beat `verse`, on its `bass` track.
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

/// One hit written out: middle C, one step long, at the velocity its character
/// asked for.
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

/// The fixture playing `entries` once, on a stochastic patch — so matching
/// means every hit took the ordinal it would have taken written out, noise
/// seed and all.
fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut one_pass = song();
    one_pass.arrangement = vec!["verse".into()];
    one_pass.tracks[0].patch = PatchRef::Inline(Box::new(blip()));
    verse(&mut one_pass).notes = entries;
    one_pass
}

/// `"x-xX"` as one entry, with a note after it — the trailing note is what
/// proves the string consumed **three** ordinals rather than one.
fn stepped() -> Song {
    playing(vec![
        steps("x-xX").into(),
        note("bass", "E2", 1.75, 0.25).into(),
    ])
}

/// The same bar with its hits written out one at a time.
fn spelled() -> Song {
    let mut hits = played(vec![hit(0.0, PLAIN), hit(1.0, PLAIN), hit(1.5, 1.0)]);
    hits.push(note("bass", "E2", 1.75, 0.25).into());
    playing(hits)
}

#[test]
fn a_step_string_equals_writing_its_hits_out_by_hand() {
    assert_eq!(render(&stepped()), render(&spelled()));
}

/// The performance reaches each hit separately, exactly as it reaches each
/// hand-written note — which is what expanding before the note loop buys.
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

/// Every character is a step, so one that is not a step takes the count with
/// it — and the count is what proves the string covers its bar.
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

/// The error the notation exists to make loud: a string one character short
/// reads as a bar on the page, and silent truncation would leave the ear to
/// find it.
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

/// Untagged variants are told apart by which field is present, so a step
/// string has to survive a save and come back one — and an entry that names
/// two things to play, or misspells a field, has to be refused rather than
/// read as whichever arm tolerated it.
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
        // A chord and a rhythm are two different things to play.
        r#"{ "track": "b", "chord": "Dm7", "steps": "x-xX", "div": 0.5, "start": 0, "dur": 1 }"#,
        // A misspelled field, which would otherwise be a velocity silently at 1.
        r#"{ "track": "b", "steps": "x-xX", "div": 0.5, "vell": 0.4 }"#,
        // A string with no grid says nothing about when its hits fall.
        r#"{ "track": "b", "steps": "x-xX" }"#,
    ] {
        assert!(
            serde_json::from_str::<PatternEntry>(confused).is_err(),
            "`{confused}` should not have parsed"
        );
    }
}

/// `start` is written down when it says something and left out when it does
/// not, and both halves are load-bearing.
///
/// Leaving it out is not tidiness: a bake is addressed by the hash of its
/// recipe's bytes, so a serialiser that began writing a default into every step
/// entry would invalidate every cached bake in every project at once, for no
/// change in the audio. Writing it out when it *is* something is the other
/// half, and the one whose failure is silent — a string moved back to the top
/// of its bar by a save nobody watched.
#[test]
fn a_step_entry_writes_down_its_start_only_when_it_has_one() {
    // The string alone, so the only `start` that could appear is its own —
    // every other entry kind writes one unconditionally.
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

/// A step string is written where its track is named, so a typo'd track is the
/// same silence — and the same message — as a typo'd track on a note.
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
