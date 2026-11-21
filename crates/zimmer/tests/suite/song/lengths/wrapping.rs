//! `tail: wrap`: a song that comes back round without a seam, because what
//! rang past its last beat is summed onto its first.

use super::setup::{fitted, samples};
use crate::common::peak;
use crate::common::songs::{blip, song, verse, voice};
use scorsese_zimmer::patch::{Adsr, Patch};
use scorsese_zimmer::song::{
    Excerpt, Fade, FitMode, InlineOnly, PatchRef, Pattern, Span, Tail, Window, render_excerpt,
};
use scorsese_zimmer::{Song, SynthError, render_song};

/// Both channels, interleaved, as a file holds them — the fold is the same
/// sum on each side, and asserting on one would let the other drift.
fn stereo(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// The fixture with its last note held past the final beat on an instrument
/// that sustains, so there is a real tail to fold, and turned down far enough
/// that the limiter never has to act on it.
///
/// That last part is what lets these tests see the sum **before the
/// limiter** through the public API: a signal that never approaches the
/// ceiling is left untouched, sample for sample, so what comes out is exactly
/// what was folded.
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

/// The fold is an addition, and it happens before the master limiter rather
/// than after it — so a loud song whose tail lands on a loud first bar still
/// comes out under the ceiling (−1 dBTP, and so under 0.9 at every sample).
#[test]
fn a_loud_wrapped_song_still_does_not_clip() {
    let mut loud = ringing(Tail::Wrap);
    loud.tracks[0].gain = 2.0;
    let rendered = stereo(&loud);
    assert!(peak(&rendered) <= 0.9, "peaked at {}", peak(&rendered));
}

/// 64 beats at 96 bpm is forty seconds, and forty seconds is 1,764,000
/// samples — not one more for the tail, not one fewer for rounding.
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

/// A tail that would still be ringing the next time round is a recipe that
/// does not loop, and it says so with both lengths rather than cutting one.
#[test]
fn a_tail_longer_than_the_loop_is_refused() {
    let mut rings_on = ringing(Tail::Wrap);
    voice(verse(&mut rings_on), 1).dur = 8.0;
    let Err(SynthError::WrapOverhang { overhang, length }) = render_song(&rings_on, &InlineOnly)
    else {
        panic!("a four-second tail on a two-second loop rendered");
    };
    assert_eq!(length, 2.0);
    assert!(overhang > 3.5, "overhang {overhang}");
    let said = SynthError::WrapOverhang { overhang, length }.to_string();
    assert!(said.contains("2.00 s loop"), "{said}");
}

/// `stretch` lands a whole number of passes on the target, so it has a loop
/// point — and the file comes out at the target, to the sample.
#[test]
fn a_stretched_song_wraps_on_its_target() {
    let stretched = Song {
        tail: Some(Tail::Wrap),
        ..fitted(4.1, FitMode::Stretch)
    };
    assert_eq!(stereo(&stretched).len(), 2 * samples(4.1));
}

/// Everything that would put a seam back into the loop is refused, before a
/// sample is rendered.
#[test]
fn what_would_break_the_loop_is_refused() {
    let wrapped = |song: Song| Song {
        tail: Some(Tail::Wrap),
        ..song
    };
    for mode in [FitMode::Loop, FitMode::Once] {
        let fitted = wrapped(fitted(4.0, mode));
        assert!(
            matches!(fitted.validate(), Err(SynthError::WrapWith { .. })),
            "{mode:?} was let through"
        );
    }
    for (in_seconds, out_seconds) in [(0.0, 1.0), (1.0, 0.0)] {
        let faded = wrapped(Song {
            fade: Some(Fade {
                in_seconds,
                out_seconds,
            }),
            ..song()
        });
        assert!(matches!(
            faded.validate(),
            Err(SynthError::WrapWith {
                field: "`fade`",
                ..
            })
        ));
    }
}

/// An excerpt's promise holds under `wrap`: the first bar of a window is the
/// first bar of the file, tail and all — which means the notes at the end
/// are rendered for it, however early the window closes.
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
