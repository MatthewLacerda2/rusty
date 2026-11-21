//! A feel the renderer refuses: a swing that would reorder the music, a humanize amount that is
//! not an amount.

use crate::common::songs::song;
use rusty_zimmer::song::{Humanize, Song};
use rusty_zimmer::SynthError;

#[test]
fn a_swing_that_would_reorder_the_music_is_refused() {
    for swing in [1.0, 1.5, -0.2] {
        let odd = Song { swing, ..song() };
        assert_eq!(odd.validate(), Err(SynthError::BadSwing { swing }));
    }
}

#[test]
fn a_humanise_amount_that_is_not_an_amount_is_refused_by_name() {
    let backwards = Humanize {
        timing: -0.01,
        ..Humanize::default()
    };
    assert_eq!(
        Song {
            humanize: Some(backwards),
            ..song()
        }
        .validate(),
        Err(SynthError::BadHumanize {
            field: "timing",
            amount: -0.01,
        })
    );

    let nonsense = Humanize {
        velocity: f32::NAN,
        ..Humanize::default()
    };
    assert!(matches!(
        Song {
            humanize: Some(nonsense),
            ..song()
        }
        .validate(),
        Err(SynthError::BadHumanize {
            field: "velocity",
            ..
        })
    ));

    let untuned = Humanize {
        timbre: -1.0,
        ..Humanize::default()
    };
    assert_eq!(
        Song {
            humanize: Some(untuned),
            ..song()
        }
        .validate(),
        Err(SynthError::BadHumanize {
            field: "timbre",
            amount: -1.0,
        })
    );
}
