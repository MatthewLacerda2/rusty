//! A degree is the note it names, and nothing else has to know.
//!
//! Both assertions here are the claim the notation lives or dies by: **writing
//! a degree must render byte-identically to writing the note it resolves to.**
//! If it does not, a document that reads as the fifth of D minor is playing
//! something the page does not say.

use super::setup::{degree, playing, render, triad};
use crate::common::songs::{note, played};
use scorsese_zimmer::song::Degree;

/// The numbering rule, end to end: `1 3 5` in D minor is D F A, not D F# A.
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

/// The alteration grammar, which is what keeps a minor key's leading tone and
/// every borrowed note writable as a degree at all.
#[test]
fn an_altered_degree_is_the_accidental_it_writes() {
    let sharpened = playing(
        Some("D minor"),
        vec![degree(Degree::Altered("#7".to_owned()), 4, 0.0)],
    );
    let spelled = playing(Some("D minor"), played(vec![note("bass", "C#5", 0.0, 0.5)]));
    assert_eq!(render(&sharpened), render(&spelled));
}
