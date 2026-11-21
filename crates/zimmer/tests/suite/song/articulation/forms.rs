//! A mark survives every way an entry can be written, and the document says so.

use super::setup::{gate, note, playing, rendered, TRACK};
use crate::common::{peak, saw_patch};
use rusty_zimmer::song::Articulation::{Accent, Ghost, Staccato};
use rusty_zimmer::song::{
    Articulation, Chord, Degree, DegreeNote, PatchRef, PatternEntry, Pitch, Song, Steps, Voicing,
};
use rusty_zimmer::SynthError;

fn forms(mark: Option<Articulation>) -> Vec<(&'static str, PatternEntry)> {
    let track = || TRACK.to_owned();
    vec![
        ("note", note("E3", 0.0, 1.0, 0.3, mark).into()),
        (
            "degree",
            DegreeNote {
                track: track(),
                degree: Degree::Plain(1),
                oct: Some(3),
                start: 0.0,
                dur: 1.0,
                vel: 0.3,
                articulation: mark,
            }
            .into(),
        ),
        (
            "chord",
            Chord {
                track: track(),
                chord: Voicing::Name("Em".to_owned()),
                oct: Some(3),
                start: 0.0,
                dur: 1.0,
                vel: 0.3,
                articulation: mark,
                arp: None,
                div: None,
                gate: None,
                inv: None,
                voicing: None,
            }
            .into(),
        ),
        ("steps", string("x---", mark).into()),
    ]
}

fn string(steps: &str, mark: Option<Articulation>) -> Steps {
    Steps {
        track: TRACK.to_owned(),
        steps: steps.to_owned(),
        div: 1.0,
        start: 0.0,
        dur: Some(1.0),
        note: Some(Pitch::Name("E3".to_owned())),
        vel: 0.3,
        articulation: mark,
    }
}

fn played(entry: PatternEntry) -> Vec<f32> {
    let mut song = playing(vec![entry]);
    song.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
    rendered(&song)
}

#[test]
fn every_written_form_carries_its_mark_through_expansion() {
    for ((name, plain), (_, ghosted)) in forms(None).into_iter().zip(forms(Some(Ghost))) {
        let (plain, ghosted) = (played(plain), played(ghosted));
        let level = peak(&ghosted) / peak(&plain);
        assert!((level - 0.35).abs() < 0.02, "`{name}` came out at {level}");
        assert!(gate(&ghosted) * 3 < gate(&plain) * 2, "`{name}` is as long");
    }
}

#[test]
fn an_unmarked_note_writes_nothing_into_the_document() {
    let song = playing(vec![note("E3", 0.0, 1.0, 0.3, None).into()]);
    let json = song.to_json().expect("the song serialises");
    assert!(!json.contains("articulation"), "{json}");
    assert_eq!(Song::from_json(&json).expect("reads back"), song);
}

#[test]
fn a_mark_round_trips_as_the_word_it_is_written_with() {
    let song = playing(vec![note("E3", 0.0, 1.0, 0.3, Some(Staccato)).into()]);
    let json = song.to_json().expect("the song serialises");
    assert!(json.contains("\"articulation\": \"staccato\""), "{json}");
    assert_eq!(Song::from_json(&json).expect("reads back"), song);
}

#[test]
fn a_step_string_cannot_be_accented_and_mark_its_own_accents() {
    let refusal = playing(vec![string("x-xX", Some(Accent)).into()]).validate();
    assert!(
        matches!(&refusal, Err(SynthError::TwiceAccented { track }) if track == TRACK),
        "{refusal:?}"
    );
    let ok = |steps, mark| playing(vec![string(steps, mark).into()]).validate();
    assert!(ok("x-x-", Some(Accent)).is_ok(), "no `X` to argue with");
    assert!(ok("x-xX", Some(Ghost)).is_ok(), "another mark entirely");
    assert!(ok("x-xX", None).is_ok(), "the string as it always was");
}

#[test]
fn a_ghost_step_is_the_plain_hit_played_as_a_ghost() {
    let ghost = played(string("x---", Some(Ghost)).into());
    assert_eq!(played(string("o---", None).into()), ghost);
    assert_eq!(played(string("o---", Some(Staccato)).into()), ghost);
    assert_eq!(played(string("o---", Some(Accent)).into()), ghost);
}

#[test]
fn a_step_string_cannot_be_ghosted_and_mark_its_own_ghosts() {
    let refusal = playing(vec![string("x-xo", Some(Ghost)).into()]).validate();
    assert!(
        matches!(&refusal, Err(SynthError::TwiceGhosted { track }) if track == TRACK),
        "{refusal:?}"
    );
    let ok = |steps, mark| playing(vec![string(steps, mark).into()]).validate();
    assert!(ok("x-x-", Some(Ghost)).is_ok(), "no `o` to argue with");
    assert!(ok("X-xo", None).is_ok(), "all three hits in one bar");
}
