//! What `tail: wrap` refuses: a tail longer than the loop, and a fit or fade that would break it.

use crate::common::songs::{song, verse, voice};
use rusty_zimmer::song::{Fade, FitMode, InlineOnly, Tail};
use rusty_zimmer::{render_song, Song, SynthError};

use super::{fitted, ringing};

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
