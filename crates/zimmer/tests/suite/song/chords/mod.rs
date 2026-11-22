//! A chord as one entry, held to the notes it replaced.

use crate::common::saw_patch;
use crate::common::songs::{blip, note, played, song, verse};
use rusty_zimmer::song::{Chord, InlineOnly, Note, PatchRef, PatternEntry, Pitch, Voicing};
use rusty_zimmer::{render_song, Song, SynthError};

mod played;

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

fn block(name: &str, oct: Option<i32>) -> Chord {
    Chord {
        track: "bass".to_owned(),
        chord: Voicing::Name(name.to_owned()),
        oct,
        start: 0.0,
        dur: 1.0,
        vel: 0.9,
        articulation: None,
        arp: None,
        div: None,
        gate: None,
        inv: None,
        voicing: None,
    }
}

fn chord(name: &str, oct: Option<i32>) -> PatternEntry {
    block(name, oct).into()
}

fn voice(pitch: Pitch) -> Note {
    Note {
        track: "bass".to_owned(),
        note: pitch,
        start: 0.0,
        dur: 1.0,
        vel: 0.9,
        articulation: None,
    }
}

fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut one_pass = song();
    one_pass.arrangement = vec!["verse".into()];
    one_pass.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
    one_pass.tracks[0].gain = 0.15;
    verse(&mut one_pass).notes = entries;
    one_pass
}

/// The trailing note proves the chord took four ordinals, not one: a note's seed comes from its
/// ordinal.
fn spelled(names: [&str; 4]) -> Song {
    let mut voices: Vec<PatternEntry> = names
        .iter()
        .map(|name| voice(Pitch::Name((*name).to_owned())).into())
        .collect();
    voices.push(note("bass", "E2", 1.0, 0.5).into());
    playing(voices)
}

fn chorded(name: &str) -> Song {
    playing(vec![
        chord(name, Some(3)),
        note("bass", "E2", 1.0, 0.5).into(),
    ])
}

#[test]
fn a_chord_equals_writing_its_voices_out_by_hand() {
    // A stochastic instrument on purpose: matching here means every voice took the ordinal it would
    // have taken written out, noise seed and all.
    let noisy = |mut song: Song| {
        song.tracks[0].patch = PatchRef::Inline(Box::new(blip()));
        render(&song)
    };
    assert_eq!(
        noisy(chorded("Dm7")),
        noisy(spelled(["D3", "F3", "A3", "C4"]))
    );
}

#[test]
fn a_name_off_the_table_is_refused_rather_than_guessed() {
    let wrong = chorded("Cmaj13");
    assert!(
        matches!(wrong.validate(), Err(SynthError::UnknownChord { chord }) if chord == "Cmaj13"),
        "an unknown chord name must be refused, and by name"
    );
}

#[test]
fn a_chord_spelled_as_pitches_plays_those_pitches() {
    let spread = Chord {
        chord: Voicing::Pitches(vec![
            Pitch::Name("D2".to_owned()),
            Pitch::Name("A3".to_owned()),
            Pitch::Midi(60.0),
        ]),
        ..block("unused", None)
    };
    let by_hand = playing(played(vec![
        voice(Pitch::Name("D2".to_owned())),
        voice(Pitch::Name("A3".to_owned())),
        voice(Pitch::Name("C4".to_owned())),
    ]));
    assert_eq!(render(&playing(vec![spread.into()])), render(&by_hand));
}
