//! What swing, humanise, a track's own feel and a delay timed in beats are
//! refused with: where a note or an echo lands against the beat.
//!
//! Out of [`super`] because that file is at the size gate, and this is a
//! group with one subject — the song clock and what sits on it. The parent's
//! match hands every one of these variants here by name, as it does
//! [`super::length`]'s, and the catch-all below is there for the same reason
//! that one's is.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    /// The words for a timing refusal. Only ever called from
    /// [`SynthError::say`] with one of the five variants it names.
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
            // `say` hands over only the five above; see `length`'s catch-all.
            other => write!(f, "{other:?}"),
        }
    }
}
