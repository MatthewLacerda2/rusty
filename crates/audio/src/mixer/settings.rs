use serde::{Deserialize, Serialize};

use super::effects::{ChainPatch, EffectChain};

pub const LOW_PASS_OFF: f32 = 22_000.0;
/// A high-pass cutoff at or below this is "off" (Unity's default, 10 Hz).
pub const HIGH_PASS_OFF: f32 = 10.0;
/// The cutoff range a filter accepts, in Hz.
const CUTOFF_RANGE: (f32, f32) = (HIGH_PASS_OFF, LOW_PASS_OFF);

/// A filter's cutoff (Hz) and resonance (`0..1`, `0` is a plain slope).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub cutoff: f32,
    pub resonance: f32,
}

impl Filter {
    /// A filter clamped into range.
    pub fn new(cutoff: f32, resonance: f32) -> Self {
        Self {
            cutoff: cutoff.clamp(CUTOFF_RANGE.0, CUTOFF_RANGE.1),
            resonance: resonance.clamp(0.0, 1.0),
        }
    }

    pub const LOW_PASS_OPEN: Filter = Filter {
        cutoff: LOW_PASS_OFF,
        resonance: 0.0,
    };

    pub const HIGH_PASS_OPEN: Filter = Filter {
        cutoff: HIGH_PASS_OFF,
        resonance: 0.0,
    };

    fn lerp(self, to: Filter, t: f32) -> Filter {
        let (a, b) = (self.cutoff.max(1.0).log2(), to.cutoff.max(1.0).log2());
        let cutoff = match t {
            _ if self.cutoff == to.cutoff => to.cutoff,
            t if t >= 1.0 => to.cutoff,
            t => (a + (b - a) * t).exp2(),
        };
        Filter {
            cutoff,
            resonance: lerp(self.resonance, to.resonance, t),
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupSettings {
    /// Linear gain in `[0, 1]`.
    pub volume: f32,
    pub mute: bool,
    pub low_pass: Filter,
    pub high_pass: Filter,
    pub reverb_send: f32,
    pub effects: EffectChain,
}

impl Default for GroupSettings {
    /// Unity's fresh group: unity gain, unmuted, both filters open, no send.
    fn default() -> Self {
        Self {
            volume: 1.0,
            mute: false,
            low_pass: Filter::LOW_PASS_OPEN,
            high_pass: Filter::HIGH_PASS_OPEN,
            reverb_send: 0.0,
            effects: EffectChain::default(),
        }
    }
}

impl GroupSettings {
    pub fn lerp(&self, to: &GroupSettings, t: f32) -> GroupSettings {
        GroupSettings {
            volume: lerp(self.volume, to.volume, t),
            mute: to.mute,
            low_pass: self.low_pass.lerp(to.low_pass, t),
            high_pass: self.high_pass.lerp(to.high_pass, t),
            reverb_send: lerp(self.reverb_send, to.reverb_send, t),
            effects: self.effects.lerp(&to.effects, t),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupPatch {
    pub volume: Option<f32>,
    pub mute: Option<bool>,
    pub low_pass: Option<Filter>,
    pub high_pass: Option<Filter>,
    pub reverb_send: Option<f32>,
    pub effects: Option<EffectChain>,
    pub effect_params: Option<ChainPatch>,
}

impl GroupPatch {
    /// `settings` with every named field replaced (clamped into range).
    pub fn apply(&self, settings: &GroupSettings) -> GroupSettings {
        let low_pass = self.low_pass.unwrap_or(settings.low_pass);
        let high_pass = self.high_pass.unwrap_or(settings.high_pass);
        let effects = self.effects.unwrap_or(settings.effects);
        GroupSettings {
            volume: self.volume.unwrap_or(settings.volume).clamp(0.0, 1.0),
            mute: self.mute.unwrap_or(settings.mute),
            low_pass: Filter::new(low_pass.cutoff, low_pass.resonance),
            high_pass: Filter::new(high_pass.cutoff, high_pass.resonance),
            reverb_send: self
                .reverb_send
                .unwrap_or(settings.reverb_send)
                .clamp(0.0, 1.0),
            effects: self.effect_params.map_or(effects, |p| p.apply(&effects)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupMix {
    pub volume: f32,
    pub low_pass: Filter,
    pub high_pass: Filter,
    pub reverb_send: f32,
    pub effects: EffectChain,
}

impl GroupMix {
    pub fn new(settings: &GroupSettings, duck: f32) -> Self {
        let volume = if settings.mute {
            0.0
        } else {
            settings.volume * duck
        };
        Self {
            volume,
            low_pass: settings.low_pass,
            high_pass: settings.high_pass,
            reverb_send: settings.reverb_send,
            effects: settings.effects,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupState {
    pub name: String,
    pub parent: Option<String>,
    pub settings: GroupSettings,
    /// The gain ducking applies right now (`1` = not ducked).
    pub duck: f32,
    pub effective_volume: f32,
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
