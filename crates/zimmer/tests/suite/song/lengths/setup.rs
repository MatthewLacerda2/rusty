//! Shared setup for the length-and-level tests.

use scorsese_zimmer::song::{Fit, FitMode, InlineOnly};
use scorsese_zimmer::{SAMPLE_RATE, Song, render_song};

use crate::common::channel;
use crate::common::songs::song;

/// Renders a song whose instruments are all inline, as one channel of it —
/// every question below is about length or level, and both sides carry the
/// same one.
pub(crate) fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

/// Samples in `seconds`, the way the renderer counts them.
pub(crate) fn samples(seconds: f32) -> usize {
    (seconds * SAMPLE_RATE as f32).round() as usize
}

/// The fixture with a target length attached.
///
/// The fixture is 4 beats at 120 bpm — two seconds — so every target in these
/// tests is a deliberate multiple or non-multiple of that.
pub(crate) fn fitted(seconds: f32, mode: FitMode) -> Song {
    Song {
        fit: Some(Fit::lasting(seconds, mode)),
        ..song()
    }
}
