//! Rendering less of a song: a window, and a solo.

use crate::common::saw_patch;
use crate::common::songs::{note, song, verse};
use rusty_zimmer::patch::Fx;
use rusty_zimmer::song::{Fit, FitMode, InlineOnly, PatchRef, Song, Track};
use rusty_zimmer::{render_excerpt, render_song, Excerpt, Span, Window};

const RATE: f32 = 44_100.0;
const CHANNELS: usize = 2;

fn whole(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

fn part(song: &Song, excerpt: &Excerpt) -> Vec<f32> {
    render_excerpt(song, &InlineOnly, excerpt).expect("the excerpt renders")
}

fn at(seconds: f32) -> usize {
    (seconds * RATE).round() as usize * CHANNELS
}

fn seconds(from: f32, to: f32) -> Excerpt {
    Excerpt::of(Window::seconds(
        Span::new(from, Some(to)).expect("a legal span"),
    ))
}

fn mixed() -> Song {
    let mut song = song();
    song.tracks.push(Track {
        name: "pad".to_owned(),
        patch: PatchRef::Inline(Box::new(saw_patch())),
        gain: 0.7,
        pan: -0.4,
        fx: vec![Fx::Compress {
            threshold: -30.0,
            ratio: 8.0,
            attack: 0.05,
            release: 0.1,
            makeup: 0.0,
            mix: 1.0,
            sidechain: Some("bass".to_owned()),
        }],
        send: 1.0,
        push: 0.0,
        swing: None,
        humanize: None,
    });
    verse(&mut song)
        .notes
        .push(note("pad", "E3", 0.0, 2.0).into());
    song.fx = vec![Fx::Reverb {
        size: 0.7,
        damp: 0.4,
        mix: 0.3,
        predelay: 0.0,
    }];
    song
}

fn looped() -> Song {
    let mut song = mixed();
    song.fit = Some(Fit::lasting(6.0, FitMode::Loop));
    song
}

#[test]
fn a_window_is_exactly_that_stretch_of_the_whole_bake() {
    for song in [mixed(), looped()] {
        let all = whole(&song);
        // (0, 0.98) pins the guard: the pad's compressor, keyed from the bass struck at 1 s, looks
        // ahead by its 50 ms attack and ducks inside it.
        for (from, to) in [(0.0, 1.0), (0.75, 2.25), (0.0, 0.98), (2.0, 3.5)] {
            let cut = part(&song, &seconds(from, to));
            assert_eq!(
                cut,
                all[at(from)..at(to)],
                "seconds {from}..{to} of the window are not the ones in the whole"
            );
        }
    }
}

#[test]
fn a_window_in_beats_counts_along_the_rendered_piece() {
    let song = looped();
    let all = whole(&song);
    let window = Window::beats(Span::new(8.0, Some(12.0)).expect("a legal span"));
    let cut = part(&song, &Excerpt::of(window));
    // 120 bpm, so a beat is half a second and this is seconds 4 to 6.
    assert_eq!(cut, all[at(4.0)..at(6.0)], "beats 8..12 are seconds 4..6");
}

#[test]
fn an_open_window_runs_to_the_end_of_the_piece() {
    let song = mixed();
    let all = whole(&song);
    let cut = part(
        &song,
        &Excerpt::of(Window::beats(Span::new(2.0, None).expect("a legal span"))),
    );
    assert_eq!(cut, all[at(1.0)..], "beat 2 onwards is second 1 onwards");
}

#[test]
fn a_solo_is_the_mix_of_the_tracks_it_names() {
    let song = mixed();
    let only_pad = part(&song, &Excerpt::only(vec!["pad".to_owned()]));
    assert!(
        only_pad.iter().any(|sample| sample.abs() > 0.01),
        "the soloed pad is silent"
    );
    assert_ne!(only_pad, whole(&song), "a solo is not the whole mix");
}

#[test]
fn a_solo_still_plays_the_track_its_compressor_listens_to() {
    let song = mixed();
    let ducked = part(&song, &Excerpt::only(vec!["pad".to_owned()]));
    let mut deaf = song.clone();
    deaf.tracks[1].fx.clear();
    let unducked = part(&deaf, &Excerpt::only(vec!["pad".to_owned()]));
    assert_ne!(ducked, unducked, "the kick stopped ducking the soloed pad");
}

#[test]
fn a_solo_of_a_track_that_does_not_exist_is_refused() {
    let refused = render_excerpt(
        &mixed(),
        &InlineOnly,
        &Excerpt::only(vec!["strings".to_owned()]),
    )
    .expect_err("there is no such track");
    assert!(
        format!("{refused}").contains("strings"),
        "the refusal does not name the track: {refused}"
    );
}
