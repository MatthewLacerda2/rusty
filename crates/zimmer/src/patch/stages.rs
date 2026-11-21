//! The stages a patch is built from: what makes the tone, and what shapes it.

use serde::{Deserialize, Serialize};

use super::fm::{Algorithm, Operator, FM_OPERATORS};

pub const MAX_OSCS: usize = 4;

pub const MAX_VOICES: usize = 7;

pub const MAX_PARTIALS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wave {
    Sine,
    Triangle,
    Saw,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseColor {
    #[default]
    White,
    Pink,
    Brown,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Osc {
    pub wave: Wave,
    #[serde(default)]
    pub detune_cents: f32,
    #[serde(default = "one")]
    pub gain: f32,
    #[serde(default)]
    pub octave: i32,
    #[serde(default = "solo", skip_serializing_if = "is_solo")]
    pub voices: usize,
    #[serde(default = "spread_default", skip_serializing_if = "is_default_spread")]
    pub spread: f32,
    #[serde(default, skip_serializing_if = "is_centred")]
    pub width: f32,
    #[serde(default = "half", skip_serializing_if = "is_half")]
    pub pulse_width: f32,
}

pub const MIN_PULSE_WIDTH: f32 = 0.05;

pub const MAX_PULSE_WIDTH: f32 = 0.95;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Partial {
    pub ratio: f32,
    #[serde(default = "one")]
    pub gain: f32,
    #[serde(default)]
    pub detune_cents: f32,
    #[serde(default)]
    pub decay: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    OscStack {
        oscs: Vec<Osc>,
    },
    Karplus {
        #[serde(default = "damping_default")]
        damping: f32,
        #[serde(default = "half")]
        brightness: f32,
    },
    Noise {
        #[serde(default, skip_serializing_if = "is_white")]
        color: NoiseColor,
    },
    Fm2 {
        ratio: f32,
        index: f32,
        #[serde(default)]
        vel_index: f32,
        #[serde(default = "mod_decay_default")]
        mod_decay: f32,
    },
    Fm4 {
        algorithm: Algorithm,
        operators: [Operator; FM_OPERATORS],
        #[serde(default)]
        vel_index: f32,
    },
    Additive {
        partials: Vec<Partial>,
    },
}

impl Source {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OscStack { .. } => "osc_stack",
            Self::Karplus { .. } => "karplus",
            Self::Noise { .. } => "noise",
            Self::Fm2 { .. } => "fm2",
            Self::Fm4 { .. } => "fm4",
            Self::Additive { .. } => "additive",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adsr {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
    #[serde(default, skip_serializing_if = "is_linear")]
    pub curve: f32,
}

fn is_linear(curve: &f32) -> bool {
    *curve == 0.0
}

impl Default for Adsr {
    fn default() -> Self {
        Self {
            a: 0.005,
            d: 0.0,
            s: 1.0,
            r: 0.05,
            curve: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PitchEnv {
    pub semitones: f32,
    #[serde(default)]
    pub adsr: Adsr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slope {
    #[default]
    #[serde(rename = "12db")]
    Db12,
    #[serde(rename = "24db")]
    Db24,
}

impl Slope {
    pub(crate) fn pole_pairs(self) -> usize {
        match self {
            Self::Db12 => 1,
            Self::Db24 => 2,
        }
    }
}

fn gentle(slope: &Slope) -> bool {
    matches!(slope, Slope::Db12)
}

fn untracked(key_track: &f32) -> bool {
    *key_track == 0.0
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub kind: FilterKind,
    pub cutoff: f32,
    #[serde(default, skip_serializing_if = "gentle")]
    pub slope: Slope,
    #[serde(default)]
    pub resonance: f32,
    #[serde(default)]
    pub env_octaves: f32,
    #[serde(default)]
    pub vel_octaves: f32,
    #[serde(default, skip_serializing_if = "untracked")]
    pub key_track: f32,
    #[serde(default)]
    pub adsr: Adsr,
}

pub(super) fn one() -> f32 {
    1.0
}

fn is_white(color: &NoiseColor) -> bool {
    matches!(color, NoiseColor::White)
}

fn solo() -> usize {
    1
}

fn is_solo(voices: &usize) -> bool {
    *voices == 1
}

fn spread_default() -> f32 {
    12.0
}

fn is_default_spread(spread: &f32) -> bool {
    *spread == spread_default()
}

fn is_centred(width: &f32) -> bool {
    *width == 0.0
}

fn half() -> f32 {
    0.5
}

fn is_half(pulse_width: &f32) -> bool {
    *pulse_width == half()
}

fn damping_default() -> f32 {
    0.996
}

fn mod_decay_default() -> f32 {
    0.3
}
