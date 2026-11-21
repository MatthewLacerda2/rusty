//! Shared setup for the arrangement-transform tests.

use rusty_zimmer::song::{InlineOnly, Note, Pattern, PatternEntry, Play};
use rusty_zimmer::{render_song, Song};

use crate::common::channel;

pub(crate) fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

pub(crate) fn play(pattern: &str) -> Play {
    Play {
        pattern: pattern.to_owned(),
        transpose: None,
        transpose_degrees: None,
        vel_scale: None,
        tracks: None,
    }
}

pub(crate) fn alongside(song: &mut Song, notes: Vec<Note>) {
    let notes = notes.into_iter().map(PatternEntry::from).collect();
    song.patterns
        .insert("higher".to_owned(), Pattern { beats: 2.0, notes });
}
