//! A song whose tempo moves: the changes written after `bpm`.

use serde::{Deserialize, Serialize};

use crate::error::SynthError;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoChange {
    pub beat: f32,
    pub bpm: f32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ramp: bool,
}

impl TempoChange {
    pub const fn jump(beat: f32, bpm: f32) -> Self {
        Self {
            beat,
            bpm,
            ramp: false,
        }
    }

    pub const fn ramp(beat: f32, bpm: f32) -> Self {
        Self {
            beat,
            bpm,
            ramp: true,
        }
    }
}

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
