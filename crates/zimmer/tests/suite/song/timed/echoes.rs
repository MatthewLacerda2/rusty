//! A delay timed in `beats`: the seconds it works out to at the tempo the
//! song is actually rendered at — after a `stretch` fit has moved it.

use crate::common::songs::song;
use scorsese_zimmer::patch::Fx;
use scorsese_zimmer::song::{Fit, FitMode, InlineOnly, TempoChange};
use scorsese_zimmer::{Song, render_song};

/// A delay, in seconds or in beats.
fn echo(time: f32, beats: Option<f32>) -> Fx {
    Fx::Delay {
        time,
        beats,
        feedback: 0.4,
        mix: 0.5,
        ping_pong: false,
    }
}

/// The fixture with `fx` on its one track's chain.
fn on_the_track(fx: Fx) -> Song {
    let mut song = song();
    song.tracks[0].fx = vec![fx];
    song
}

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// Half a beat at 120 bpm is a quarter of a second, to the sample.
#[test]
fn half_a_beat_at_120_is_a_quarter_second() {
    let beats = on_the_track(echo(0.0, Some(0.5)));
    let seconds = on_the_track(echo(0.25, None));
    assert_eq!(render(&beats), render(&seconds));
    // And on the song's own chain, which the sum runs through.
    let mut summed = song();
    summed.fx = vec![echo(0.0, Some(0.5))];
    let mut timed = song();
    timed.fx = vec![echo(0.25, None)];
    assert_eq!(render(&summed), render(&timed));
}

/// Stretched from two seconds to two and a half, the fixture plays at 96 bpm:
/// half a beat is now 0.3125 s, and the echo moves with the music where a
/// fixed `time` of a quarter second would have drifted off the grid.
#[test]
fn a_stretched_song_moves_its_echoes_with_it() {
    let stretched = |fx| Song {
        fit: Some(Fit::lasting(2.5, FitMode::Stretch)),
        ..on_the_track(fx)
    };
    let beats = render(&stretched(echo(0.0, Some(0.5))));
    assert_eq!(beats, render(&stretched(echo(0.3125, None))));
    assert_ne!(beats, render(&stretched(echo(0.25, None))));
}

/// Under a tempo map the echo follows the map: after the tempo halves, half a
/// beat is half a second, and the delay is not the one the opening tempo
/// alone would have played.
#[test]
fn a_tempo_map_moves_the_echo_where_the_tempo_moves() {
    let mapped = |fx| Song {
        tempo: vec![TempoChange::jump(2.0, 60.0)],
        ..on_the_track(fx)
    };
    let beats = render(&mapped(echo(0.0, Some(0.5))));
    assert!(beats.iter().all(|sample| sample.is_finite()));
    assert_ne!(beats, render(&mapped(echo(0.25, None))));
}
