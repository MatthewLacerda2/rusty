//! Shared setup for the arrangement-transform tests.

use scorsese_zimmer::song::{InlineOnly, Note, Pattern, PatternEntry, Play};
use scorsese_zimmer::{Song, render_song};

use crate::common::channel;

/// Renders a song whose instruments are all inline, as one channel of it — a
/// transform changes what is played, never where it is played from.
pub(crate) fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

/// The long form of `pattern`, with nothing changed about it — the base every
/// test spreads its one transform over.
pub(crate) fn play(pattern: &str) -> Play {
    Play {
        pattern: pattern.to_owned(),
        transpose: None,
        transpose_degrees: None,
        vel_scale: None,
        tracks: None,
    }
}

/// A second two-beat pattern named `higher`, for the by-hand half of a
/// comparison — the same slot length as the fixture's `verse`.
pub(crate) fn alongside(song: &mut Song, notes: Vec<Note>) {
    let notes = notes.into_iter().map(PatternEntry::from).collect();
    song.patterns
        .insert("higher".to_owned(), Pattern { beats: 2.0, notes });
}
