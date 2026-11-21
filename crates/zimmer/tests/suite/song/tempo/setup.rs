//! Shared setup for the tempo map tests.

use scorsese_zimmer::render_song;
use scorsese_zimmer::song::{InlineOnly, Song, TempoChange};

use crate::common::songs::song;

/// Sample-frames per second.
pub(crate) const RATE: f32 = 44_100.0;

/// The fixture song — two two-beat verses at 120 bpm, a noise blip on every
/// beat — with its tempo halved on beat 2. The first verse lasts a second
/// and the second two, so each note of the second verse is a second apart.
pub(crate) fn slowing() -> Song {
    Song {
        tempo: vec![TempoChange::jump(2.0, 60.0)],
        ..song()
    }
}

/// The left channel of `song`, rendered whole.
pub(crate) fn left(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly)
        .expect("the song renders")
        .chunks(2)
        .map(|frame| frame[0])
        .collect()
}

/// The first frame at or after `from` seconds that is not silence.
pub(crate) fn onset(buf: &[f32], from: f32) -> usize {
    let start = frame(from);
    start
        + buf[start..]
            .iter()
            .position(|sample| *sample != 0.0)
            .expect("something sounds after that point")
}

/// The frame `seconds` into the piece.
pub(crate) fn frame(seconds: f32) -> usize {
    (seconds * RATE).round() as usize
}

/// Every section's end, in seconds, as [`Song::sections`] reports them.
pub(crate) fn ends(song: &Song) -> Vec<f64> {
    song.sections().iter().map(|cut| cut.end_seconds).collect()
}

/// Whether two lists of seconds agree to within a microsecond each.
pub(crate) fn close(got: &[f64], want: &[f64]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-5)
}
