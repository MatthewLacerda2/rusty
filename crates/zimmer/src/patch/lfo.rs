//! The low-frequency oscillators: the movement a held note gets.
//!
//! A patch carries up to [`MAX_LFOS`] of them, in `lfo` and `lfo2`, each
//! tapping one target from the closed [`LfoTarget`] list. Two aimed at the
//! same target add: pitch in semitones, cutoff in octaves, pulse width in
//! width, and the amp's two dips multiply. No LFO moves another LFO, an
//! envelope's time or an fx parameter: a modulation matrix would be a free
//! graph, which is exactly the shape a patch deliberately is not.

use serde::{Deserialize, Serialize};

use crate::error::SynthError;

/// The most LFOs a patch may carry: `lfo` and `lfo2`. Two is a pad that both
/// breathes and shimmers; a third is where a patch starts being a matrix.
pub const MAX_LFOS: usize = 2;

/// What the LFO modulates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoTarget {
    /// Vibrato: `depth` semitones of pitch bend either way.
    Pitch,
    /// Filter wobble: `depth` octaves of cutoff sweep either way.
    Cutoff,
    /// Tremolo: amplitude dips by `depth` (1.0 dips to silence).
    Amp,
    /// Pulse-width modulation: every `square` in the stack swings `depth`
    /// either way around its written
    /// [`Osc::pulse_width`](super::Osc::pulse_width), clamped to the range.
    /// Refused on a patch with no square to move.
    PulseWidth,
}

/// The wave an LFO traces, each over one cycle and in `−1..=1`.
///
/// Every shape but `square` starts its cycle at zero and rises, as the sine
/// does, so a quarter of the way through is the top and three quarters the
/// bottom: changing the shape changes the path, not where the movement begins.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoShape {
    /// The smooth swing: vibrato, a breathing filter.
    #[default]
    Sine,
    /// Straight lines up and down: a siren.
    Triangle,
    /// Rises steadily then drops back: a rhythmic upward sweep.
    SawUp,
    /// Falls steadily then jumps back: the same sweep downward.
    SawDown,
    /// The top for the first half of the cycle, the bottom for the second: a
    /// trill, or a gated tremolo.
    Square,
    /// Sample-and-hold: a new value drawn each cycle from the note's seed and
    /// held for that cycle — the random filter burble, a robot's chatter.
    /// Deterministic, like every draw in this crate.
    Random,
}

/// One low-frequency oscillator tapping one target.
///
/// Its speed is written as `rate` in Hz **or** as `beats`, the length of one
/// cycle in beats of the song — exactly one of the two. A `rate` LFO starts
/// its cycle on every note, at the end of its `delay`; a `beats` LFO takes
/// its phase from the **song position**, so a wobble stays on the grid
/// whichever note retriggers it, and a patch played on its own (which has no
/// tempo) refuses it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lfo {
    /// Rate in Hz, typically 0.1–10. Zero (and absent) when `beats` sets the
    /// speed instead.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rate: f32,
    /// The length of one cycle in beats of the song, written instead of
    /// `rate`: `1` wobbles once a beat, `0.5` twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beats: Option<f32>,
    /// Modulation depth; its unit depends on [`LfoTarget`].
    pub depth: f32,
    /// Which stage of the signal path it bends.
    pub target: LfoTarget,
    /// The wave it traces. A sine when absent.
    #[serde(default, skip_serializing_if = "is_sine")]
    pub shape: LfoShape,
    /// Seconds into the note before it moves anything. Zero when absent.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub delay: f32,
    /// Seconds after `delay` to grow from nothing to full depth — vibrato
    /// arriving on a held note. Zero (full depth at once) when absent.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fade: f32,
}

impl Lfo {
    /// A sine at `rate` Hz, moving `target` by `depth` from the note's first
    /// sample: what an LFO was before it had a shape, an onset or a tempo.
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

    /// Rejects an LFO the renderer cannot honour. `timed` says whether it
    /// plays inside a song, the one place a beat has a length.
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
