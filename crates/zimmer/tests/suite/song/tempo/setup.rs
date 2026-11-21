//! Shared setup for the tempo map tests.

use scorsese_zimmer::render_song;
use scorsese_zimmer::song::{InlineOnly, Song, TempoChange};

use crate::common::songs::song;

pub(crate) const RATE: f32 = 44_100.0;

pub(crate) fn slowing() -> Song {
    Song {
        tempo: vec![TempoChange::jump(2.0, 60.0)],
        ..song()
    }
}

pub(crate) fn left(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly)
        .expect("the song renders")
        .chunks(2)
        .map(|frame| frame[0])
        .collect()
}

pub(crate) fn onset(buf: &[f32], from: f32) -> usize {
    let start = frame(from);
    start
        + buf[start..]
            .iter()
            .position(|sample| *sample != 0.0)
            .expect("something sounds after that point")
}

pub(crate) fn frame(seconds: f32) -> usize {
    (seconds * RATE).round() as usize
}

pub(crate) fn ends(song: &Song) -> Vec<f64> {
    song.sections().iter().map(|cut| cut.end_seconds).collect()
}

pub(crate) fn close(got: &[f64], want: &[f64]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-5)
}
