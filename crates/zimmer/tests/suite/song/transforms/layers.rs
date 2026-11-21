//! Layered entries: several patterns in one slot, each written once (#506).
//!
//! The claim is the module's own, one level up: **a layered entry must equal
//! writing its layers out as one pattern by hand.** A solo over a groove is
//! then the same music whether the groove was copied into the solo or named
//! beside it — only the second keeps the groove in one place.

use crate::common::songs::{note, played, song};
use scorsese_zimmer::Song;
use scorsese_zimmer::song::{ArrangementEntry, Layer, Layers, Pattern, Play, Track};

use super::setup::{play, render};

/// The fixture's bass `verse` as the groove, a `solo` on a second track over
/// it, and `both` — the two written out as one pattern, groove first, which is
/// the order a layered entry walks in.
fn band(solo_beats: f32) -> Song {
    let mut band = song();
    band.tracks.push(Track {
        name: "lead".to_owned(),
        ..band.tracks[0].clone()
    });
    let solo = vec![note("lead", "B4", 0.5, 0.4), note("lead", "D5", 1.5, 0.4)];
    let mut both = band.patterns["verse"].clone();
    both.notes.extend(played(solo.clone()));
    band.patterns.insert(
        "solo".to_owned(),
        Pattern {
            beats: solo_beats,
            notes: played(solo),
        },
    );
    band.patterns.insert("both".to_owned(), both);
    band
}

fn layered(layers: Vec<Layer>) -> ArrangementEntry {
    Layers { layers }.into()
}

#[test]
fn a_layered_entry_equals_its_layers_written_out_as_one_pattern() {
    let mut by_hand = band(2.0);
    by_hand.arrangement = vec!["both".into(), "verse".into()];
    let mut layers = band(2.0);
    layers.arrangement = vec![layered(vec!["verse".into(), "solo".into()]), "verse".into()];

    assert_eq!(render(&layers), render(&by_hand));
}

/// One layer is the bare entry: the form is additive, and wrapping a name in
/// `layers` changes nothing it plays.
#[test]
fn a_single_layer_plays_exactly_what_the_bare_name_plays() {
    let mut bare = song();
    bare.arrangement = vec!["verse".into(), "verse".into()];
    let mut wrapped = song();
    wrapped.arrangement = vec![layered(vec!["verse".into()]), "verse".into()];

    assert_eq!(render(&wrapped), render(&bare));
}

/// A layer's transforms are its own: the solo goes up an octave and the
/// groove under it does not move.
#[test]
fn each_layer_is_transformed_alone() {
    let mut by_hand = band(2.0);
    let both = by_hand.patterns.get_mut("both").expect("defined above");
    both.notes.truncate(2);
    both.notes.extend(played(vec![
        note("lead", "B5", 0.5, 0.4),
        note("lead", "D6", 1.5, 0.4),
    ]));
    by_hand.arrangement = vec!["both".into()];

    let mut layers = band(2.0);
    let up = Play {
        transpose: Some(12.0),
        ..play("solo")
    };
    layers.arrangement = vec![layered(vec!["verse".into(), up.into()])];

    assert_eq!(render(&layers), render(&by_hand));
}

/// The slot is as long as its longest layer, so what follows it waits for
/// everything in it — and the report still has one row for it, named after
/// both.
#[test]
fn the_slot_is_its_longest_layer_and_one_section() {
    let mut layers = band(4.0);
    layers.arrangement = vec![layered(vec!["verse".into(), "solo".into()]), "verse".into()];

    assert_eq!(layers.arrangement_beats(), 6.0);
    let sections: Vec<(String, f64)> = layers
        .sections()
        .into_iter()
        .map(|cut| (cut.label, cut.end_seconds))
        .collect();
    assert_eq!(
        sections,
        [("verse + solo".to_owned(), 2.0), ("verse".to_owned(), 3.0)]
    );
}
