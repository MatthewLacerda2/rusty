//! `swing` and `humanize`: a song played rather than clocked; `humanize` has its own module.

use crate::common::channel;
use crate::common::songs::{song, verse, voice};
use rusty_zimmer::song::InlineOnly;
use rusty_zimmer::{render_song, Song};

mod humanize;

fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

#[test]
fn swinging_equals_writing_the_off_beats_late() {
    let mut swung = song();
    voice(verse(&mut swung), 1).start = 0.5;
    swung.swing = 0.5;

    let mut by_hand = song();
    voice(verse(&mut by_hand), 1).start = 0.75;

    assert_eq!(render(&swung), render(&by_hand));
}

#[test]
fn a_note_on_the_beat_does_not_move_however_hard_the_song_swings() {
    let straight = song();
    let on_the_beat = straight.patterns["verse"]
        .notes
        .iter()
        .all(|note| note.start().fract() == 0.0);
    assert!(on_the_beat, "the fixture is written on the beat");

    let swung = Song {
        swing: 0.66,
        ..song()
    };
    assert_eq!(render(&swung), render(&straight));
}
