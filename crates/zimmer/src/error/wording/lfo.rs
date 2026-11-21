//! What an LFO's speed and onset are refused with.
//!
//! Out of [`super`] for the reason [`super::pulse`] is: the parent's match
//! hands these variants here by name, and the catch-all below is there for
//! the same reason that one's is.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    /// The words for an LFO refusal. Only ever called from
    /// [`SynthError::say`] with one of the variants it names.
    pub(super) fn say_lfo(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LfoTwoRates { rate, beats } => write!(
                f,
                "patch: lfo writes both `rate` ({rate} Hz) and `beats` ({beats}) — keep one: `rate` for a speed in Hz, `beats` for one cycle per that many beats of the song"
            ),
            Self::LfoNoRate => write!(
                f,
                "patch: lfo has no speed — write a positive `rate` in Hz, or `beats` for one cycle per that many beats of the song"
            ),
            Self::BadLfoBeats { beats } => write!(
                f,
                "patch: lfo `beats` must be a positive number of beats, got {beats}"
            ),
            Self::LfoBeatsWithoutTempo { beats } => write!(
                f,
                "patch: lfo is timed in `beats` ({beats}), and a patch played on its own has no tempo to count them by — write `rate` in Hz, or play it in a song"
            ),
            Self::BadLfoOnset { field, seconds } => write!(
                f,
                "patch: lfo `{field}` must be zero or more seconds, got {seconds}"
            ),
            // `say` hands over only the variants above; see `length`'s catch-all.
            other => write!(f, "{other:?}"),
        }
    }
}
