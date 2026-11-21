//! `transpose`: the same music, somewhere else on the keyboard.

use crate::common::peak;
use crate::common::songs::{note, song, verse, voice};
use scorsese_zimmer::song::{ArrangementEntry, Note, Pitch, Play};

use super::setup::{alongside, play, render};

#[test]
fn a_bare_name_and_its_empty_long_form_are_the_same_document() {
    let mut spelled_out = song();
    spelled_out.arrangement = vec![play("verse").into(), play("verse").into()];
    assert_eq!(
        render(&spelled_out),
        render(&song()),
        "an entry that transforms nothing must render as the name alone"
    );
}

/// The strongest form the claim takes: transposing an entry produces the same
/// samples as writing every note of that pattern a fifth higher.
#[test]
fn transposing_an_entry_equals_writing_the_notes_higher() {
    let mut transposed = song();
    transposed.arrangement = vec![
        ArrangementEntry::from(Play {
            transpose: Some(7.0),
            ..play("verse")
        }),
        "verse".into(),
    ];

    let mut by_hand = song();
    alongside(
        &mut by_hand,
        vec![note("bass", "B2", 0.0, 0.5), note("bass", "F#3", 1.0, 0.5)],
    );
    by_hand.arrangement = vec!["higher".into(), "verse".into()];

    assert_eq!(render(&transposed), render(&by_hand));
}

/// Applied to the *resolved* MIDI value, so it does not care how a pitch was
/// written and a microtonal offset survives it.
#[test]
fn transposing_works_on_a_midi_pitch_as_it_does_on_a_name() {
    let mut written = song();
    voice(verse(&mut written), 0).note = Pitch::Midi(40.5);

    let mut up = written.clone();
    up.arrangement = vec![
        ArrangementEntry::from(Play {
            transpose: Some(2.0),
            ..play("verse")
        }),
        "verse".into(),
    ];

    let mut by_hand = written.clone();
    alongside(
        &mut by_hand,
        vec![
            Note {
                note: Pitch::Midi(42.5),
                ..note("bass", "E2", 0.0, 0.5)
            },
            note("bass", "C#3", 1.0, 0.5),
        ],
    );
    by_hand.arrangement = vec!["higher".into(), "verse".into()];

    assert_eq!(render(&up), render(&by_hand));
}

/// Clamped rather than refused, so whether a transpose is legal never depends
/// on the register of a pattern written months earlier.
#[test]
fn a_transpose_past_the_top_of_the_range_is_clamped_rather_than_refused() {
    let mut high = song();
    voice(verse(&mut high), 0).note = Pitch::Name("G9".to_owned());
    high.arrangement = vec![ArrangementEntry::from(Play {
        transpose: Some(24.0),
        ..play("verse")
    })];
    assert!(peak(&render(&high)) > 0.0, "it still makes a sound");
}

/// Transforms do not stack: each entry transforms the *written* pattern, never
/// what the entry before it produced. Otherwise the tenth repeat is nine
/// octaves up and the document no longer says what it does.
#[test]
fn transforms_do_not_accumulate_down_the_arrangement() {
    let up_a_fifth = ArrangementEntry::from(Play {
        transpose: Some(7.0),
        ..play("verse")
    });
    let mut twice = song();
    twice.arrangement = vec![up_a_fifth.clone(), up_a_fifth];

    let mut by_hand = song();
    alongside(
        &mut by_hand,
        vec![note("bass", "B2", 0.0, 0.5), note("bass", "F#3", 1.0, 0.5)],
    );
    by_hand.arrangement = vec!["higher".into(), "higher".into()];

    assert_eq!(render(&twice), render(&by_hand));
}
