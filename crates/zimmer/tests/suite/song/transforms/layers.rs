//! Layered entries: several patterns in one slot, each written once.

use crate::common::songs::{note, played, song};
use rusty_zimmer::song::{ArrangementEntry, Layer, Layers, Pattern, Play, Track};
use rusty_zimmer::Song;

use super::setup::{play, render};

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

#[test]
fn a_single_layer_plays_exactly_what_the_bare_name_plays() {
    let mut bare = song();
    bare.arrangement = vec!["verse".into(), "verse".into()];
    let mut wrapped = song();
    wrapped.arrangement = vec![layered(vec!["verse".into()]), "verse".into()];

    assert_eq!(render(&wrapped), render(&bare));
}

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
