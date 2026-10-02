//! src/audio/mixer/settings.rs — one mixer group's settings, and how they blend (#465).
//!
//! A group carries Unity's `AudioMixerGroup` subset: a volume, a mute, a low-pass and
//! a high-pass filter (cutoff + resonance) and a send level to the reverb bus. A
//! [`GroupPatch`] is a partial set of those — what a snapshot or a `SetGroup*` call
//! changes — so a snapshot that only muffles the world leaves the music slider alone.

use serde::{Deserialize, Serialize};

/// A low-pass cutoff at or above this is "off" (Unity's default, 22 kHz).
pub const LOW_PASS_OFF: f32 = 22_000.0;
/// A high-pass cutoff at or below this is "off" (Unity's default, 10 Hz).
pub const HIGH_PASS_OFF: f32 = 10.0;
/// The cutoff range a filter accepts, in Hz.
pub const CUTOFF_RANGE: (f32, f32) = (HIGH_PASS_OFF, LOW_PASS_OFF);

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

    /// The low-pass filter that passes everything.
    pub const LOW_PASS_OPEN: Filter = Filter {
        cutoff: LOW_PASS_OFF,
        resonance: 0.0,
    };

    /// The high-pass filter that passes everything.
    pub const HIGH_PASS_OPEN: Filter = Filter {
        cutoff: HIGH_PASS_OFF,
        resonance: 0.0,
    };

    /// Blend toward `to` by `t`: the cutoff moves in octaves (log frequency), so a
    /// sweep from 22 kHz to 500 Hz sounds even rather than all at the end.
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

/// One group's own settings (before ducking and its parents).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupSettings {
    /// Linear gain in `[0, 1]`.
    pub volume: f32,
    pub mute: bool,
    pub low_pass: Filter,
    pub high_pass: Filter,
    /// Linear send level to the reverb bus in `[0, 1]` (`0` sends nothing).
    pub reverb_send: f32,
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
        }
    }
}

impl GroupSettings {
    /// Blend toward `to` by `t` in `[0, 1]`: volume and send linearly, cutoffs in
    /// octaves. Mute is a switch, so it takes `to`'s value for the whole blend.
    pub fn lerp(&self, to: &GroupSettings, t: f32) -> GroupSettings {
        GroupSettings {
            volume: lerp(self.volume, to.volume, t),
            mute: to.mute,
            low_pass: self.low_pass.lerp(to.low_pass, t),
            high_pass: self.high_pass.lerp(to.high_pass, t),
            reverb_send: lerp(self.reverb_send, to.reverb_send, t),
        }
    }
}

/// A partial [`GroupSettings`]: the fields a snapshot or a `SetGroup*` call names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupPatch {
    pub volume: Option<f32>,
    pub mute: Option<bool>,
    pub low_pass: Option<Filter>,
    pub high_pass: Option<Filter>,
    pub reverb_send: Option<f32>,
}

impl GroupPatch {
    /// `settings` with every named field replaced (clamped into range).
    pub fn apply(&self, settings: &GroupSettings) -> GroupSettings {
        let low_pass = self.low_pass.unwrap_or(settings.low_pass);
        let high_pass = self.high_pass.unwrap_or(settings.high_pass);
        GroupSettings {
            volume: self.volume.unwrap_or(settings.volume).clamp(0.0, 1.0),
            mute: self.mute.unwrap_or(settings.mute),
            low_pass: Filter::new(low_pass.cutoff, low_pass.resonance),
            high_pass: Filter::new(high_pass.cutoff, high_pass.resonance),
            reverb_send: self
                .reverb_send
                .unwrap_or(settings.reverb_send)
                .clamp(0.0, 1.0),
        }
    }
}

/// What the device applies to one group's track: its own volume with mute and
/// ducking folded in (parents apply theirs on their own tracks), filters and send.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupMix {
    pub volume: f32,
    pub low_pass: Filter,
    pub high_pass: Filter,
    pub reverb_send: f32,
}

impl GroupMix {
    /// The mix of a group at `settings`, ducked to `duck` (`1` = not ducked).
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
        }
    }
}

/// A group as the agent reads it back (`Audio.GetGroupState`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupState {
    pub name: String,
    /// The parent group's name (`None` for Master).
    pub parent: Option<String>,
    pub settings: GroupSettings,
    /// The gain ducking applies right now (`1` = not ducked).
    pub duck: f32,
    /// What the group's voices are scaled by: its own and every ancestor's volume,
    /// mute and duck multiplied (before the master volume).
    pub effective_volume: f32,
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
