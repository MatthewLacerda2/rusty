//! A song whose tempo moves: the changes written after `bpm`.

use serde::{Deserialize, Serialize};

use crate::error::SynthError;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoChange {
    pub beat: f32,
    pub bpm: f32,
    /// Moves the tempo linearly per beat, not per second, from the previous point to this one.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ramp: bool,
}

impl TempoChange {
    /// Only zimmer's tests call it.
    pub const fn jump(beat: f32, bpm: f32) -> Self {
        Self {
            beat,
            bpm,
            ramp: false,
        }
    }

    /// Only zimmer's tests call it.
    pub const fn ramp(beat: f32, bpm: f32) -> Self {
        Self {
            beat,
            bpm,
            ramp: true,
        }
    }
}

/// The last change is not checked against the arrangement: a map may outlast a shortened piece.
pub(super) fn check(changes: &[TempoChange]) -> Result<(), SynthError> {
    let mut previous = 0.0;
    for (index, change) in changes.iter().enumerate() {
        let TempoChange { beat, bpm, .. } = *change;
        if !(beat.is_finite() && beat > 0.0 && bpm.is_finite() && bpm > 0.0) {
            return Err(SynthError::BadTempoChange { index, beat, bpm });
        }
        if beat <= previous {
            return Err(SynthError::TempoOutOfOrder {
                index,
                beat,
                previous,
            });
        }
        previous = beat;
    }
    Ok(())
}

fn is_false(ramp: &bool) -> bool {
    !*ramp
}
