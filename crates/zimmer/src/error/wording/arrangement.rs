//! What an arrangement entry's transposition and velocity, and a pattern's length, are refused with.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_arrangement(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadTranspose {
                pattern, transpose, ..
            } => write!(
                f,
                "song: arrangement entry for `{pattern}`: transpose must be finite, got {transpose}"
            ),
            Self::BadVelocityScale { pattern, scale, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: vel_scale must be >= 0, got {scale}"
            ),
            Self::BadPatternBeats { pattern, beats, .. } => write!(
                f,
                "song: pattern `{pattern}`: beats must be positive, got {beats}"
            ),
            Self::DiatonicWithoutKey { pattern, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: `transpose_degrees` needs the song to declare a `key` — or use `transpose` for a chromatic shift"
            ),
            Self::TwoTransposes { pattern, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: `transpose` is chromatic and `transpose_degrees` moves within the key — write one or the other"
            ),
            other => other.say_pattern(f),
        }
    }
}
