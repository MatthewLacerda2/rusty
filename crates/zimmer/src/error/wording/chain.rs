//! What a filter, an EQ and a compressor's key are refused with.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_chain(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadCutoff { cutoff, .. } => write!(
                f,
                "patch: filter `cutoff` must be positive Hz, got {cutoff}"
            ),
            Self::TooManyEqBands { found, limit, .. } => {
                write!(f, "fx: `eq` takes at most {limit} bands, got {found}")
            }
            Self::UnknownSidechain { track, key, .. } => write!(
                f,
                "song: track `{track}` is keyed from `{key}`, which is not a track in this song"
            ),
            Self::SelfSidechain { track, .. } => write!(
                f,
                "song: track `{track}` is keyed from itself — leave `sidechain` out for a compressor that listens to its own part"
            ),
            Self::MisplacedSidechain { place, key, .. } => write!(
                f,
                "{place}: `compress` is keyed from `{key}`, and only a track's own chain sits where one track can listen to another"
            ),
            _ => unreachable!("`say_chain` is handed only the refusals it words"),
        }
    }
}
