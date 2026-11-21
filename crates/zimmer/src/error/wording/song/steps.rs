//! What a step string is refused with.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(in crate::error) fn say_steps(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadStep {
                track,
                character,
                step,
                ..
            } => write!(
                f,
                "song: track `{track}`: `{character}` at step {step} is not a step — use `x` (a hit), `X` (an accent), `o` (a ghost) or `-` (a rest), and nothing else"
            ),
            Self::StepsDoNotFit {
                track,
                start,
                div,
                beats,
                written,
                needed,
                ..
            } => write!(
                f,
                "song: track `{track}`: {written} steps of {div} beats from beat {start} do not fill the pattern's {beats} — {needed} would"
            ),
            Self::BadStepDiv {
                track,
                start,
                div,
                beats,
                ..
            } => write!(
                f,
                "song: track `{track}`: no whole number of {div}-beat steps fills the {beats} beats from beat {start}"
            ),
            Self::AccentWithoutHeadroom { track, vel, .. } => write!(
                f,
                "song: track `{track}`: `X` and `x` are the same hit at `vel` {vel} — write a `vel` below 1 for the plain hits to be softer than the accents"
            ),
            Self::TwiceAccented { track, .. } => write!(
                f,
                "song: track `{track}`: a step string cannot be played `accent` and also mark accents with `X` — accent the whole run, or mark the hits"
            ),
            Self::TwiceGhosted { track, .. } => write!(
                f,
                "song: track `{track}`: a step string cannot be played `ghost` and also mark ghosts with `o` — ghost the whole run, or mark the hits"
            ),
            other => write!(f, "{other:?}"),
        }
    }
}
