//! A degree is the note it names, and nothing else has to know.

use super::setup::{degree, playing, render, triad};
use crate::common::songs::{note, played};
use rusty_zimmer::song::Degree;

#[test]
fn a_degree_renders_as_the_note_it_names_in_the_key() {
    let degrees = playing(
        Some("D minor"),
        vec![
            degree(Degree::Plain(1), 4, 0.0),
            degree(Degree::Plain(3), 4, 0.5),
            degree(Degree::Plain(5), 4, 1.0),
        ],
    );
    assert_eq!(
        render(&degrees),
        render(&playing(Some("D minor"), triad(["D4", "F4", "A4"])))
    );
}

#[test]
fn an_altered_degree_is_the_accidental_it_writes() {
    let sharpened = playing(
        Some("D minor"),
        vec![degree(Degree::Altered("#7".to_owned()), 4, 0.0)],
    );
    let spelled = playing(Some("D minor"), played(vec![note("bass", "C#5", 0.0, 0.5)]));
    assert_eq!(render(&sharpened), render(&spelled));
}
