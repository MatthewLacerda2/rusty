//! What swing, humanise, a track's own feel and a delay timed in beats are refused with: where a
//! note or an echo lands against the beat.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_timing(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadSwing { swing, .. } => write!(
                f,
                "song: `swing` must be at least 0 and below 1 (0 is straight, 0.33 swings), got {swing}"
            ),
            Self::BadHumanize { field, amount, .. } => write!(
                f,
                "song: `humanize.{field}` must be zero or more, got {amount}"
            ),
            Self::BadTrackFeel {
                track,
                field,
                amount,
            } => write!(
                f,
                "song: track `{track}` has `{field}` {amount} — `swing` is at least 0 and below 1, each `humanize` amount zero or more, and `push` a number of seconds"
            ),
            Self::MisplacedBeats { place, beats } => write!(
                f,
                "{place}: `delay` is timed in `beats` ({beats}), and a patch has no tempo to count them by — give it `time` in seconds, or put the delay on a track's or the song's chain"
            ),
            Self::DelayTwoTimes { time, beats } => write!(
                f,
                "fx: `delay` writes both `time` ({time} s) and `beats` ({beats}) — write one or the other"
            ),
            other => write!(f, "{other:?}"),
        }
    }
}
