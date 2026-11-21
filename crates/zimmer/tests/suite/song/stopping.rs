//! Stopping a bake part way (#661).
//!
//! The hook is additive, so the claim that matters most is the dull one: a
//! `stop` that never says yes bakes exactly what `bake_song` bakes. The other
//! two are that it is honoured — before anything, and part way through.

use std::cell::Cell;

use crate::common::songs::song;
use scorsese_zimmer::song::InlineOnly;
use scorsese_zimmer::{Excerpt, bake_excerpt_unless, bake_song};

#[test]
fn a_stop_that_never_says_yes_bakes_the_song_bit_for_bit() {
    let song = song();
    let asked = Cell::new(0u32);
    let stop = || {
        asked.set(asked.get() + 1);
        false
    };
    let kept = bake_excerpt_unless(&song, &InlineOnly, &Excerpt::default(), &stop)
        .expect("renders")
        .expect("never stopped");
    assert_eq!(kept, bake_song(&song, &InlineOnly).expect("renders"));
    assert!(asked.get() > 1, "asked between notes, not once");
}

#[test]
fn a_stop_already_asked_for_renders_nothing() {
    let stopped = bake_excerpt_unless(&song(), &InlineOnly, &Excerpt::default(), &|| true)
        .expect("not an error");
    assert!(stopped.is_none());
}

#[test]
fn a_stop_asked_for_part_way_abandons_the_bake() {
    // Trips on its second question: after the first note, before the second.
    let asked = Cell::new(0u32);
    let stop = || {
        asked.set(asked.get() + 1);
        asked.get() > 1
    };
    let stopped =
        bake_excerpt_unless(&song(), &InlineOnly, &Excerpt::default(), &stop).expect("no error");
    assert!(stopped.is_none());
    assert_eq!(asked.get(), 2, "and asks nothing after it has stopped");
}
