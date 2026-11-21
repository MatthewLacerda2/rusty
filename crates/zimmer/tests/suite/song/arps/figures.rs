//! What each word plays, stated as the pitches it plays.

use super::setup::{arped, playing, same, scattered, spelled};
use scorsese_zimmer::song::{Arp, Chord, Humanize};

#[test]
fn an_arpeggio_equals_writing_its_notes_out_by_hand() {
    let two = "D3 F3 A3 C4 D3 F3 A3 C4";
    same(arped("Dm7", Some(Arp::Up), Some(0.5), None), two, 0.5, 0.5);
}

#[test]
fn a_figure_repeats_until_the_chord_is_over_and_truncates_there() {
    let cut = "E3 G3 B3 E3 G3 B3 E3 G3";
    same(arped("Em", Some(Arp::Up), Some(0.5), None), cut, 0.5, 0.5);
}

#[test]
fn down_and_up_down_walk_the_voices_the_page_says_they_do() {
    let down = arped("Dm7", Some(Arp::Down), Some(1.0), None);
    same(down, "C4 A3 F3 D3", 1.0, 1.0);
    let turn = "D3 F3 A3 C4 A3 F3 D3 F3";
    same(
        arped("Dm7", Some(Arp::UpDown), Some(0.5), None),
        turn,
        0.5,
        0.5,
    );
}

#[test]
fn a_chord_the_length_of_one_traversal_plays_it_once() {
    let strum = Chord {
        dur: 2.0,
        ..arped("Dm7", Some(Arp::Up), Some(0.5), None)
    };
    same(strum, "D3 F3 A3 C4", 0.5, 0.5);
}

#[test]
fn a_gate_is_one_step_unless_the_chord_writes_one() {
    let held = arped("Dm7", Some(Arp::Up), Some(1.0), Some(2.0));
    same(held, "D3 F3 A3 C4", 1.0, 2.0);
}

#[test]
fn every_note_of_a_figure_is_played_rather_than_clocked() {
    let feel = Humanize {
        timing: 0.4,
        velocity: 0.3,
        timbre: 0.2,
    };
    let figure = arped("Dm7", Some(Arp::Up), Some(0.5), None);
    assert_eq!(
        scattered(playing(vec![figure.into()]), feel),
        scattered(playing(spelled("D3 F3 A3 C4 D3 F3 A3 C4", 0.5, 0.5)), feel)
    );
}
