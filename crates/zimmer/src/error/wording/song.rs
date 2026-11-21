//! What a song's tempo, tracks and arrangement as a whole are refused with.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_song(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadBpm { bpm, .. } => write!(f, "song: bpm must be positive, got {bpm}"),
            Self::NoTracks => write!(f, "song: no tracks"),
            Self::EmptyArrangement => {
                write!(f, "song: arrangement is empty — nothing would be rendered")
            }
            Self::NoLayers => write!(
                f,
                "song: arrangement entry has an empty `layers` — name at least one pattern, or write a rest as a pattern with no notes"
            ),
            Self::UnknownPattern { pattern, .. } => write!(
                f,
                "song: arrangement names pattern `{pattern}`, which is not defined"
            ),
            Self::UnknownTrackFilter { pattern, track, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: no track named `{track}` to play"
            ),
            Self::UnknownSoloTrack { track } => {
                write!(f, "song: no track named `{track}` to render on its own")
            }
            Self::BadTempoChange { index, beat, bpm } => write!(
                f,
                "song: `tempo[{index}]` must be after beat 0 (the tempo there is `bpm`) and at a positive bpm, got beat {beat} at {bpm} bpm"
            ),
            Self::TempoOutOfOrder {
                index,
                beat,
                previous,
            } => write!(
                f,
                "song: `tempo[{index}]` is at beat {beat}, not after the change before it at beat {previous} — tempo changes must ascend"
            ),
            other => other.say_arrangement(f),
        }
    }
}
