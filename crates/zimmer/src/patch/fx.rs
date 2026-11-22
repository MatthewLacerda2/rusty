//! The fx chain a patch, a track or the song is put through.

use serde::{Deserialize, Serialize};

use super::stages::one;
use crate::level::bands;

/// A high-pass, both shelves and a few peaks; past that an EQ is a filter bank run per sample.
pub const MAX_EQ_BANDS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EqKind {
    HighPass,
    LowShelf,
    Peak,
    HighShelf,
    LowPass,
}

impl EqKind {
    /// The pass filters ignore `gain_db`, so `fx::eq`'s zero-gain bypass must not skip them.
    pub fn takes_gain(self) -> bool {
        matches!(self, Self::LowShelf | Self::Peak | Self::HighShelf)
    }

    pub fn crossover(self) -> f32 {
        match self {
            Self::HighPass | Self::LowShelf | Self::Peak => bands::LOW_HZ,
            Self::HighShelf | Self::LowPass => bands::HIGH_HZ,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EqBand {
    pub kind: EqKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freq: Option<f32>,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default = "gentle_q")]
    pub q: f32,
}

impl EqBand {
    pub fn hz(&self) -> f32 {
        self.freq.unwrap_or_else(|| self.kind.crossover())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fx", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fx {
    Delay {
        #[serde(default, skip_serializing_if = "no_time")]
        time: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        beats: Option<f32>,
        feedback: f32,
        mix: f32,
        #[serde(default, skip_serializing_if = "not_ping_pong")]
        ping_pong: bool,
    },
    Reverb {
        size: f32,
        damp: f32,
        mix: f32,
        #[serde(default, skip_serializing_if = "no_predelay")]
        predelay: f32,
    },
    Saturate {
        drive: f32,
        mix: f32,
    },
    Compress {
        threshold: f32,
        ratio: f32,
        #[serde(default = "attack_default")]
        attack: f32,
        #[serde(default = "release_default")]
        release: f32,
        #[serde(default)]
        makeup: f32,
        #[serde(default = "one")]
        mix: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sidechain: Option<String>,
    },
    Chorus {
        rate: f32,
        depth: f32,
        #[serde(default = "voices_default")]
        voices: usize,
        mix: f32,
    },
    Eq {
        bands: Vec<EqBand>,
    },
}

fn not_ping_pong(ping_pong: &bool) -> bool {
    !*ping_pong
}

fn no_time(time: &f32) -> bool {
    *time == 0.0
}

fn no_predelay(predelay: &f32) -> bool {
    *predelay == 0.0
}

fn gentle_q() -> f32 {
    0.707
}

fn attack_default() -> f32 {
    0.01
}

fn release_default() -> f32 {
    0.15
}

fn voices_default() -> usize {
    3
}
