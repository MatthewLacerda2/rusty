//! The low-frequency oscillators: the movement a held note gets.

use serde::{Deserialize, Serialize};

use crate::error::SynthError;

/// The `lfo` and `lfo2` fields; a third would start a modulation matrix, which a patch is not.
pub const MAX_LFOS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoTarget {
    Pitch,
    Cutoff,
    Amp,
    PulseWidth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    SawUp,
    SawDown,
    Square,
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lfo {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rate: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beats: Option<f32>,
    pub depth: f32,
    pub target: LfoTarget,
    #[serde(default, skip_serializing_if = "is_sine")]
    pub shape: LfoShape,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub delay: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fade: f32,
}

impl Lfo {
    /// Test hook: only rusty-zimmer's unit tests and its `tests/suite` call it.
    pub fn new(rate: f32, depth: f32, target: LfoTarget) -> Self {
        Self {
            rate,
            beats: None,
            depth,
            target,
            shape: LfoShape::Sine,
            delay: 0.0,
            fade: 0.0,
        }
    }

    pub(super) fn check(&self, timed: bool) -> Result<(), SynthError> {
        if self.rate < 0.0 {
            return Err(SynthError::NegativeLfoRate { rate: self.rate });
        }
        match self.beats {
            Some(beats) if self.rate != 0.0 => Err(SynthError::LfoTwoRates {
                rate: self.rate,
                beats,
            }),
            Some(beats) if !(beats.is_finite() && beats > 0.0) => {
                Err(SynthError::BadLfoBeats { beats })
            }
            Some(beats) if !timed => Err(SynthError::LfoBeatsWithoutTempo { beats }),
            None if self.rate == 0.0 || !self.rate.is_finite() => Err(SynthError::LfoNoRate),
            _ => Ok(()),
        }?;
        for (field, seconds) in [("delay", self.delay), ("fade", self.fade)] {
            if !(seconds.is_finite() && seconds >= 0.0) {
                return Err(SynthError::BadLfoOnset { field, seconds });
            }
        }
        Ok(())
    }
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

fn is_sine(shape: &LfoShape) -> bool {
    *shape == LfoShape::Sine
}
