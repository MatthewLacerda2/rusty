//! What a track's own feel and a delay in beats are refused for.

use crate::common::saw_patch;
use crate::common::songs::song;
use scorsese_zimmer::SynthError;
use scorsese_zimmer::patch::{Fx, Patch};
use scorsese_zimmer::song::Humanize;

/// Each field a track can write, at a value that stops meaning what it says.
#[test]
fn a_track_feel_that_is_not_one_is_refused_by_name() {
    let refused = |edit: &dyn Fn(&mut scorsese_zimmer::Song)| {
        let mut song = song();
        edit(&mut song);
        song.validate().expect_err("refused")
    };
    let feel = |field: &'static str, amount: f32| SynthError::BadTrackFeel {
        track: "bass".to_owned(),
        field,
        amount,
    };
    assert_eq!(
        refused(&|it| it.tracks[0].push = f32::INFINITY),
        feel("push", f32::INFINITY)
    );
    assert_eq!(
        refused(&|it| it.tracks[0].swing = Some(1.0)),
        feel("swing", 1.0)
    );
    let wild = Humanize {
        velocity: -0.1,
        ..Humanize::default()
    };
    assert_eq!(
        refused(&|it| it.tracks[0].humanize = Some(wild)),
        feel("humanize.velocity", -0.1)
    );
    let mut fine = song();
    fine.tracks[0].push = -0.02;
    fine.tracks[0].swing = Some(0.33);
    assert_eq!(fine.validate(), Ok(()));
}

/// Beats on a patch, which has no tempo, and two spacings on one delay.
#[test]
fn a_delay_in_beats_is_refused_where_it_cannot_be_counted() {
    let delay = |time, beats| Fx::Delay {
        time,
        beats,
        feedback: 0.3,
        mix: 0.4,
        ping_pong: false,
    };
    let patch = Patch {
        fx: vec![delay(0.0, Some(0.5))],
        ..saw_patch()
    };
    assert_eq!(
        patch.validate(),
        Err(SynthError::MisplacedBeats {
            place: "patch",
            beats: 0.5
        })
    );
    let mut both = song();
    both.tracks[0].fx = vec![delay(0.2, Some(0.5))];
    assert_eq!(
        both.validate(),
        Err(SynthError::DelayTwoTimes {
            time: 0.2,
            beats: 0.5
        })
    );
    both.tracks[0].fx = vec![delay(0.0, Some(0.5))];
    assert_eq!(both.validate(), Ok(()));
}
