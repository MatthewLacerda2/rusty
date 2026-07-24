//! src/soundgen/patch.rs — the patch document: one instrument, as serde data
//! (#357).
//!
//! A [`Patch`] is the "Minimoog-lite" description of a single sound. It is
//! **structured, not a free graph** (the #272 shape, not #270's DAG): the signal
//! path is fixed — `source → filter → amp envelope → fx` with an LFO tapping one
//! target — because a patch must honour a contract, *playable as a note*. The agent
//! chooses *what goes in each stage*, never how the stages connect.
//!
//! Patch-as-truth: plain serde data, no buffers and no handles, so it round-trips
//! losslessly and is the same shape whether it came from a `.json` on disk or a Lua
//! table. [`crate::soundgen::core`] renders it; [`crate::soundgen::bake`] writes the
//! WAV.

use serde::{Deserialize, Serialize};

/// The most oscillators one stack may carry. Four detuned saws is already a fat
/// analog lead; past that it is CPU for no audible gain.
pub const MAX_OSCS: usize = 4;

/// One oscillator's waveform. `saw` and `square` are band-limited (polyBLEP) at
/// render time — the naive versions alias audibly at high pitch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wave {
    Sine,
    Triangle,
    Saw,
    Square,
}

/// One oscillator in a stack: its wave, its detune from the played pitch, its
/// weight in the mix, and a whole-octave transpose.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Osc {
    pub wave: Wave,
    /// Detune in cents (100 cents = a semitone). Small opposing detunes across a
    /// stack are what make a "fat" lead.
    #[serde(default)]
    pub detune_cents: f32,
    /// Weight in the stack mix; the stack is normalized by the sum of gains.
    #[serde(default = "one")]
    pub gain: f32,
    /// Whole-octave transpose, e.g. `-1` for a sub-oscillator.
    #[serde(default)]
    pub octave: i32,
}

/// What generates the raw tone. Exactly one per patch — the head of the fixed
/// signal path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// Subtractive stack: up to [`MAX_OSCS`] oscillators summed, the classic
    /// analog-synth head.
    OscStack { oscs: Vec<Osc> },
    /// Karplus-Strong plucked string: a noise-excited delay line with damped
    /// feedback. Guitars, basses, harps, marimbas.
    Karplus {
        /// Feedback scale per round trip, `0..1` — how long the string rings.
        #[serde(default = "damping_default")]
        damping: f32,
        /// Excitation brightness, `0..1`: 0 is a soft, pre-lowpassed pluck, 1 is
        /// full white noise (a hard pick).
        #[serde(default = "half")]
        brightness: f32,
    },
    /// Raw white noise — the head of every gunshot, impact and footstep, shaped by
    /// the filter and amp envelope.
    Noise,
    /// Two-operator FM: a sine modulator at `ratio × f` bending a sine carrier.
    /// Electric pianos, bells, metallic hits.
    Fm2 {
        /// Modulator frequency as a multiple of the played pitch. Integer ratios
        /// are harmonic (tonal); non-integer ratios go inharmonic (metallic).
        ratio: f32,
        /// Modulation depth. Higher indices add sidebands, i.e. brightness.
        index: f32,
        /// Seconds for the modulator's own decay — how fast the bright attack
        /// collapses to a near-sine body.
        #[serde(default = "mod_decay_default")]
        mod_decay: f32,
    },
}

/// A linear-segment ADSR envelope. Attack/decay/release are seconds; sustain is a
/// level in `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adsr {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

impl Default for Adsr {
    /// A quick, fully-sustaining envelope — an organ-ish "just pass it through".
    fn default() -> Self {
        Self {
            a: 0.005,
            d: 0.0,
            s: 1.0,
            r: 0.05,
        }
    }
}

/// Which side of the filter is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterKind {
    Lowpass,
    Highpass,
}

/// The (optional) filter stage: a Chamberlin state-variable filter whose cutoff is
/// swept by its own envelope — the move that makes a subtractive patch expressive.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub kind: FilterKind,
    /// Base cutoff in Hz.
    pub cutoff: f32,
    /// Resonance in `0..1`: emphasis at the cutoff. Near 1 the filter self-rings.
    #[serde(default)]
    pub resonance: f32,
    /// How many Hz the filter envelope adds to the cutoff at full level. Negative
    /// sweeps downward.
    #[serde(default)]
    pub env_amount: f32,
    /// The cutoff envelope. Defaults to the same fast shape as [`Adsr::default`].
    #[serde(default)]
    pub adsr: Adsr,
}

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
}

/// A single sine low-frequency oscillator tapping one target. One is enough for
/// vibrato, wobble or tremolo; a modulation matrix is not v1 scope.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lfo {
    /// Rate in Hz (typically 0.1–10).
    pub rate: f32,
    /// Modulation depth; its unit depends on [`LfoTarget`].
    pub depth: f32,
    pub target: LfoTarget,
}

/// One effect in the post-chain, applied in list order. A limiter is *always*
/// applied at bake and is deliberately not listed here — it is not a choice.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fx", rename_all = "snake_case")]
pub enum Fx {
    /// Feedback echo.
    Delay {
        /// Echo spacing in seconds.
        time: f32,
        /// How much of each echo feeds the next, `0..1`.
        feedback: f32,
        /// Wet/dry blend, `0..=1`.
        mix: f32,
    },
    /// Freeverb room reverb (8 combs + 4 allpasses).
    Reverb {
        /// Room size, `0..=1`.
        size: f32,
        /// High-frequency damping, `0..=1` — how quickly the tail dulls.
        damp: f32,
        /// Wet/dry blend, `0..=1`.
        mix: f32,
    },
}

/// One instrument. `source` and `amp` are mandatory (a sound needs a tone and a
/// shape); `filter`, `lfo` and `fx` are optional stages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub source: Source,
    pub amp: Adsr,
    #[serde(default)]
    pub filter: Option<Filter>,
    #[serde(default)]
    pub lfo: Option<Lfo>,
    #[serde(default)]
    pub fx: Vec<Fx>,
}

impl Patch {
    /// Serialize to pretty JSON — the on-disk patch form.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Parse a patch from its JSON form.
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }

    /// Reject a patch the renderer can't honour, *before* any samples are written.
    /// Only the bounds that would produce silence, a divide-by-zero or an unstable
    /// filter are checked — musical taste is the agent's business.
    pub fn validate(&self) -> Result<(), String> {
        match &self.source {
            Source::OscStack { oscs } if oscs.is_empty() => {
                return Err("patch: `osc_stack` needs at least one oscillator".into())
            }
            Source::OscStack { oscs } if oscs.len() > MAX_OSCS => {
                return Err(format!(
                    "patch: `osc_stack` takes at most {MAX_OSCS} oscillators, got {}",
                    oscs.len()
                ))
            }
            Source::OscStack { oscs } if oscs.iter().all(|o| o.gain <= 0.0) => {
                return Err("patch: every oscillator gain is zero — the stack is silent".into())
            }
            Source::Fm2 { ratio, .. } if *ratio <= 0.0 => {
                return Err("patch: `fm2` needs a positive `ratio`".into())
            }
            _ => {}
        }
        if let Some(f) = &self.filter {
            if f.cutoff <= 0.0 {
                return Err("patch: filter `cutoff` must be positive Hz".into());
            }
        }
        if let Some(l) = &self.lfo {
            if l.rate < 0.0 {
                return Err("patch: lfo `rate` must not be negative".into());
            }
        }
        Ok(())
    }
}

fn one() -> f32 {
    1.0
}

fn half() -> f32 {
    0.5
}

fn damping_default() -> f32 {
    0.996
}

fn mod_decay_default() -> f32 {
    0.3
}

#[cfg(test)]
#[path = "patch_tests.rs"]
mod patch_tests;
