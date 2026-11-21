//! A track's own feel: `push`, `swing` and `humanize` on one part in place of the song's.

use crate::common::songs::{song, verse, voice};
use crate::common::{channel, saw_patch};
use scorsese_zimmer::song::{Humanize, InlineOnly, PatchRef};
use scorsese_zimmer::{SAMPLE_RATE, Song, render_song};

fn steady() -> Song {
    let mut steady = song();
    steady.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
    steady
}

fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

#[test]
fn a_pushed_part_is_the_same_part_moved() {
    let plain = render(&steady());
    let late = 441;
    let mut back = steady();
    back.tracks[0].push = 0.01;
    let laid_back = render(&back);
    assert!(laid_back[..late].iter().all(|sample| *sample == 0.0));
    let span = plain.len() - late - 1_000;
    assert_eq!(laid_back[late..late + span], plain[..span]);

    // Ahead of the beat: everything after the first note, which has nowhere earlier to go, lands
    // that much sooner.
    let mut ahead = steady();
    ahead.tracks[0].push = -0.01;
    let pushed = render(&ahead);
    let second = (steady().beat_seconds() * SAMPLE_RATE as f32) as usize;
    assert_eq!(
        pushed[second - late..second - late + 1_000],
        plain[second..second + 1_000]
    );
}

#[test]
fn a_track_swing_is_the_song_swing_on_that_track() {
    let mut written = steady();
    voice(verse(&mut written), 1).start = 0.5;
    let mut by_track = written.clone();
    by_track.tracks[0].swing = Some(0.5);
    let by_song = Song {
        swing: 0.5,
        ..written.clone()
    };
    assert_eq!(render(&by_track), render(&by_song));
    assert_ne!(render(&by_track), render(&written));
    let mut straight = by_song.clone();
    straight.tracks[0].swing = Some(0.0);
    assert_eq!(render(&straight), render(&written));
}

#[test]
fn a_track_humanize_replaces_the_songs() {
    let loose = Song {
        humanize: Some(Humanize {
            timing: 0.03,
            velocity: 0.2,
            timbre: 0.0,
        }),
        ..steady()
    };
    let mut tight = loose.clone();
    tight.tracks[0].humanize = Some(Humanize::default());
    assert_eq!(render(&tight), render(&steady()));
    assert_ne!(render(&loose), render(&steady()));
}
