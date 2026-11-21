//! `tail: wrap`: a song that comes back round without a seam, because what rang past its last beat
//! is summed onto its first.

use super::setup::{fitted, samples};
use crate::common::peak;
use crate::common::songs::{blip, song, verse, voice};
use rusty_zimmer::patch::{Adsr, Patch};
use rusty_zimmer::song::{
    render_excerpt, Excerpt, FitMode, InlineOnly, PatchRef, Pattern, Span, Tail, Window,
};
use rusty_zimmer::{render_song, Song};

mod refusals;

fn stereo(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

fn ringing(tail: Tail) -> Song {
    let mut rings_out = song();
    let sustains = Patch {
        amp: Adsr {
            s: 0.5,
            ..blip().amp
        },
        ..blip()
    };
    rings_out.tracks[0].patch = PatchRef::Inline(Box::new(sustains));
    rings_out.tracks[0].gain = 0.1;
    voice(verse(&mut rings_out), 1).dur = 1.5;
    Song {
        tail: Some(tail),
        ..rings_out
    }
}

#[test]
fn a_wrapped_song_starts_with_its_own_tail() {
    let ring = stereo(&ringing(Tail::Ring));
    let wrap = stereo(&ringing(Tail::Wrap));
    // Two seconds of stereo: 4 beats at 120 bpm, two samples a frame.
    let length = 2 * samples(2.0);
    assert!(
        peak(&ring[length + 2 * samples(0.2)..]) > 0.01,
        "the fixture rings out, or this proves nothing"
    );
    assert!(
        peak(&ring) < 0.5 && peak(&wrap) < 0.5,
        "and the limiter never touched either"
    );
    assert_eq!(wrap.len(), length, "the file ends on the last beat");
    let overhang = ring.len() - length;
    for (index, sample) in wrap.iter().enumerate() {
        let past = if index < overhang {
            ring[length + index]
        } else {
            0.0
        };
        assert_eq!(*sample, ring[index] + past, "sample {index}");
    }
}

#[test]
fn a_loud_wrapped_song_still_does_not_clip() {
    let mut loud = ringing(Tail::Wrap);
    loud.tracks[0].gain = 2.0;
    let rendered = stereo(&loud);
    assert!(peak(&rendered) <= 0.9, "peaked at {}", peak(&rendered));
}

#[test]
fn a_wrapped_song_is_exactly_the_arrangement_long() {
    let mut long = ringing(Tail::Wrap);
    long.bpm = 96.0;
    let four_beats: &mut Pattern = verse(&mut long);
    four_beats.beats = 4.0;
    voice(four_beats, 1).dur = 4.0;
    long.arrangement = vec!["verse".into(); 16];
    let rendered = stereo(&long);
    assert_eq!(rendered.len() / 2, 1_764_000);
    assert_eq!(rendered.len(), 2 * samples(40.0));
}

#[test]
fn a_stretched_song_wraps_on_its_target() {
    let stretched = Song {
        tail: Some(Tail::Wrap),
        ..fitted(4.1, FitMode::Stretch)
    };
    assert_eq!(stereo(&stretched).len(), 2 * samples(4.1));
}

#[test]
fn a_window_on_a_wrapped_song_is_the_same_samples() {
    let wrap = ringing(Tail::Wrap);
    let whole = stereo(&wrap);
    let window = Excerpt::of(Window::beats(
        Span::new(0.0, Some(1.0)).expect("a legal span"),
    ));
    let opening = render_excerpt(&wrap, &InlineOnly, &window).expect("the window renders");
    assert_eq!(opening.len(), 2 * samples(0.5));
    assert_eq!(opening[..], whole[..opening.len()]);
}

#[test]
fn wrap_is_written_as_a_word() {
    let tail: Tail = serde_json::from_str("\"wrap\"").expect("`wrap` parses");
    assert_eq!(tail, Tail::Wrap);
}
