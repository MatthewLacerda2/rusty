//! What a pattern's notes, and a track's patch reference, are refused with.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_pattern(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTrack {
                pattern,
                index,
                track,
                ..
            } => write!(
                f,
                "song: pattern `{pattern}` note {index}: no track named `{track}`"
            ),
            Self::BadNoteStart {
                pattern,
                index,
                start,
                ..
            } => write!(
                f,
                "song: pattern `{pattern}` note {index}: start must be >= 0, got {start}"
            ),
            Self::BadNoteDuration {
                pattern,
                index,
                dur,
                ..
            } => write!(
                f,
                "song: pattern `{pattern}` note {index}: dur must be positive, got {dur}"
            ),
            Self::UnresolvedPatch {
                track,
                reference,
                reason,
                ..
            } => write!(
                f,
                "song: track `{track}`: cannot resolve patch `{reference}`: {reason}"
            ),
            other => other.say_harmony(f),
        }
    }
}
