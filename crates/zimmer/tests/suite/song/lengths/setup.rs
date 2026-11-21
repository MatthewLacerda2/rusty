//! Shared setup for the length-and-level tests.

use scorsese_zimmer::song::{Fit, FitMode, InlineOnly};
use scorsese_zimmer::{SAMPLE_RATE, Song, render_song};

use crate::common::channel;
use crate::common::songs::song;

pub(crate) fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

pub(crate) fn samples(seconds: f32) -> usize {
    (seconds * SAMPLE_RATE as f32).round() as usize
}

pub(crate) fn fitted(seconds: f32, mode: FitMode) -> Song {
    Song {
        fit: Some(Fit::lasting(seconds, mode)),
        ..song()
    }
}
