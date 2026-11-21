//! `tail` and `fade`: where the piece stops, and how it gets there.

use super::setup::{fitted, render, samples};
use crate::common::peak;
use crate::common::songs::{song, verse, voice};
use scorsese_zimmer::song::{Fade, FitMode, Tail};
use scorsese_zimmer::{Song, SynthError};

/// `exact` is for music that has to butt against something: the file ends on
/// the last beat rather than wherever the ring-out happens to stop.
#[test]
fn an_exact_tail_ends_on_the_final_beat() {
    let mut rings_out = song();
    voice(verse(&mut rings_out), 1).dur = 8.0;
    let arrangement_end = samples(4.0 * rings_out.beat_seconds());
    assert!(
        render(&rings_out).len() > arrangement_end,
        "the fixture rings out, or this proves nothing"
    );

    let exact = Song {
        tail: Some(Tail::Exact),
        ..rings_out
    };
    let rendered = render(&exact);
    assert_eq!(rendered.len(), arrangement_end);
    assert!(
        rendered.last().copied().expect("samples").abs() < 1e-3,
        "and the tail is faded into it rather than chopped"
    );
}

#[test]
fn fades_move_the_level_at_each_end() {
    let faded = Song {
        fade: Some(Fade {
            in_seconds: 0.5,
            out_seconds: 0.5,
        }),
        ..fitted(4.0, FitMode::Loop)
    };
    let rendered = render(&faded);
    let plain = render(&fitted(4.0, FitMode::Loop));
    assert_eq!(rendered.len(), samples(4.0), "a fade changes no lengths");
    // Against the unfaded render rather than an absolute level: what a fade
    // does is make the start *quieter than it would have been*, and how loud
    // that is depends on the instrument.
    let opening = samples(0.05);
    assert!(
        peak(&rendered[..opening]) < peak(&plain[..opening]) * 0.2,
        "the start was not faded down"
    );
    assert!(
        rendered.last().copied().expect("samples").abs() < 1e-3,
        "and ends silent"
    );
    assert!(
        peak(&rendered[samples(1.0)..samples(3.0)]) > 0.05,
        "with the middle left alone"
    );
}

#[test]
fn a_negative_fade_is_refused() {
    let faded = Song {
        fade: Some(Fade {
            in_seconds: -1.0,
            out_seconds: 0.0,
        }),
        ..song()
    };
    assert!(matches!(faded.validate(), Err(SynthError::BadFade { .. })));
}

/// Absent means what every song did before these fields existed.
#[test]
fn a_song_with_none_of_this_is_unchanged() {
    let plain = song();
    assert!(plain.fit.is_none() && plain.fade.is_none());
    assert_eq!(plain.tail(), Tail::Ring);
    assert_eq!(render(&plain).len(), samples(4.0 * plain.beat_seconds()));
}
