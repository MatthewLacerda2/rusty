//! A mark survives every way an entry can be written, and the document says so.
//!
//! A chord's voices, a step string's hits and a degree all become ordinary
//! notes before the mixer sees one, so the failure this file exists to catch is
//! silent: an expansion that forgets to carry the field renders a perfectly
//! good note that simply is not the one the page marked.

use super::setup::{TRACK, gate, note, playing, rendered};
use crate::common::{peak, saw_patch};
use scorsese_zimmer::SynthError;
use scorsese_zimmer::song::Articulation::{Accent, Ghost, Staccato};
use scorsese_zimmer::song::{
    Articulation, Chord, Degree, DegreeNote, PatchRef, PatternEntry, Pitch, Song, Steps, Voicing,
};

/// One beat of E3 at the same level, written each of the four ways, with the
/// mark given over it.
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

/// A step string of one hit at the top of the pattern, marked or not.
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

/// The fixture playing one entry through a plain saw, where the level is
/// exactly the velocity and a ratio between two renders means something.
fn played(entry: PatternEntry) -> Vec<f32> {
    let mut song = playing(vec![entry]);
    song.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
    rendered(&song)
}

/// Every form comes out at the ghost's own fraction of the note it wrote, and
/// shorter with it — the two halves of the mark, in all four.
#[test]
fn every_written_form_carries_its_mark_through_expansion() {
    for ((name, plain), (_, ghosted)) in forms(None).into_iter().zip(forms(Some(Ghost))) {
        let (plain, ghosted) = (played(plain), played(ghosted));
        let level = peak(&ghosted) / peak(&plain);
        assert!((level - 0.35).abs() < 0.02, "`{name}` came out at {level}");
        assert!(gate(&ghosted) * 3 < gate(&plain) * 2, "`{name}` is as long");
    }
}

/// An unmarked note writes no `articulation` at all. A bake is addressed by
/// the bytes of its recipe, so a default written into every note would miss
/// the cache for every song in every project, for no change in the audio.
#[test]
fn an_unmarked_note_writes_nothing_into_the_document() {
    let song = playing(vec![note("E3", 0.0, 1.0, 0.3, None).into()]);
    let json = song.to_json().expect("the song serialises");
    assert!(!json.contains("articulation"), "{json}");
    assert_eq!(Song::from_json(&json).expect("reads back"), song);
}

/// And a marked one round-trips as the word the page documents.
#[test]
fn a_mark_round_trips_as_the_word_it_is_written_with() {
    let song = playing(vec![note("E3", 0.0, 1.0, 0.3, Some(Staccato)).into()]);
    let json = song.to_json().expect("the song serialises");
    assert!(json.contains("\"articulation\": \"staccato\""), "{json}");
    assert_eq!(Song::from_json(&json).expect("reads back"), song);
}

/// The one combination refused: a step string already says which hits are
/// accented, so playing the whole run `accent` is the word used twice.
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

/// An `o` is the ghost mark on that one hit, not a level of its own: it plays
/// exactly as the plain hit marked `ghost` does, and the entry's own mark does
/// not reach it.
#[test]
fn a_ghost_step_is_the_plain_hit_played_as_a_ghost() {
    let ghost = played(string("x---", Some(Ghost)).into());
    assert_eq!(played(string("o---", None).into()), ghost);
    assert_eq!(played(string("o---", Some(Staccato)).into()), ghost);
    assert_eq!(played(string("o---", Some(Accent)).into()), ghost);
}

/// The same refusal as the accent's, for the other mark a string can spell.
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
