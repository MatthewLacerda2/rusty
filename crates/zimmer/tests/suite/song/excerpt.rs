//! Rendering less of a song: a window, and a solo.
//!
//! The property the whole feature stands on is here: **a window is the whole
//! bake, cut**. Sample for sample, not to within a tolerance — a mix decision
//! taken on eight bars is only worth taking if those eight bars are the ones
//! the file will have. So the songs below carry the stages that could break
//! it: a compressor keyed off another track and one on the sum, both of which
//! look ahead, a reverb that decays across the window's edges, a `fit` that
//! changes how many times the arrangement plays, and a fade at the end.

use crate::common::saw_patch;
use crate::common::songs::{note, song, verse};
use scorsese_zimmer::patch::Fx;
use scorsese_zimmer::song::{Fit, FitMode, InlineOnly, PatchRef, Song, Track};
use scorsese_zimmer::{Excerpt, Span, Window, render_excerpt, render_song};

const RATE: f32 = 44_100.0;
const CHANNELS: usize = 2;

fn whole(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

fn part(song: &Song, excerpt: &Excerpt) -> Vec<f32> {
    render_excerpt(song, &InlineOnly, excerpt).expect("the excerpt renders")
}

/// Where a second of the piece starts in an interleaved buffer.
fn at(seconds: f32) -> usize {
    (seconds * RATE).round() as usize * CHANNELS
}

fn seconds(from: f32, to: f32) -> Excerpt {
    Excerpt::of(Window::seconds(
        Span::new(from, Some(to)).expect("a legal span"),
    ))
}

/// The fixture song with a second instrument, a sidechained compressor on it,
/// a reverb over the sum and a fade — the stages that ring, look ahead, and
/// reach the end of the piece.
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

/// The same piece looped to a length the picture asked for, which is the case
/// where "beat 40 of the arrangement" and "beat 40 of the rendered piece" are
/// different beats.
fn looped() -> Song {
    let mut song = mixed();
    song.fit = Some(Fit::lasting(6.0, FitMode::Loop));
    song
}

/// The promise, stated as plainly as it can be: what a window hands back is
/// what the whole render put there, bit for bit. Anything else is a bug in the
/// window rather than a tolerance to widen.
#[test]
fn a_window_is_exactly_that_stretch_of_the_whole_bake() {
    for song in [mixed(), looped()] {
        let all = whole(&song);
        // The third of these is the one that pins the guard: the pad's
        // compressor is keyed from the bass, which strikes on the beat at one
        // second, and its fifty-millisecond attack means the duck begins
        // *inside* a window that closes at 0.98. A render that stopped at the
        // window's own edge would miss the note that causes it.
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

/// Beats are the piece's own unit, and under a `loop` fit they count along
/// what was rendered — so beat 8 of a four-beat arrangement is in its third
/// pass, and that is what comes back.
#[test]
fn a_window_in_beats_counts_along_the_rendered_piece() {
    let song = looped();
    let all = whole(&song);
    let window = Window::beats(Span::new(8.0, Some(12.0)).expect("a legal span"));
    let cut = part(&song, &Excerpt::of(window));
    // 120 bpm, so a beat is half a second and this is seconds 4 to 6.
    assert_eq!(cut, all[at(4.0)..at(6.0)], "beats 8..12 are seconds 4..6");
}

/// An open end runs to wherever the piece stops, ring-out included.
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

/// A solo is the mix with fewer parts in it, so it is quieter than the whole
/// and is not silence — and it still runs the song's own chain, which is why
/// it is a soloed *mix* rather than a bare track.
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

/// A track something is keyed from is still played when a solo leaves it out —
/// otherwise the solo would show a duck the mix does not have.
#[test]
fn a_solo_still_plays_the_track_its_compressor_listens_to() {
    let song = mixed();
    let ducked = part(&song, &Excerpt::only(vec!["pad".to_owned()]));
    let mut deaf = song.clone();
    deaf.tracks[1].fx.clear();
    let unducked = part(&deaf, &Excerpt::only(vec!["pad".to_owned()]));
    assert_ne!(ducked, unducked, "the kick stopped ducking the soloed pad");
}

/// A name that is not a track is refused rather than rendering silence, which
/// is the only way a typo in a solo can be told from an instrument that does
/// not play here.
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
