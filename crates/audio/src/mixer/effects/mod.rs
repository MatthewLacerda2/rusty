//! crates/audio/src/mixer/effects/ — a mixer group's effects (#1748).
//!
//! Unity's mixer effects and Godot's bus effects, cut to a small fixed set: every
//! group carries an ordered [`EffectChain`] of up to [`MAX_EFFECTS`] effects, run
//! after its filters and before its volume (Master's after its volume and the
//! reverb bus, on the device's main track, #1855). The kinds are rows of [`KINDS`]
//! (`kinds/`): a compressor (with an optional sidechain from another group), a
//! limiter, an EQ, a distortion and a delay. Each row names its parameters
//! ([`Param`]) and builds the DSP that runs it on the audio thread ([`Dsp`]), so a
//! new effect is one file in `kinds/` and one row, and no shared `match` grows.
//!
//! An effect's settings are plain `Copy` data ([`EffectParams`]: its kind and its
//! values in its row's order), so they sit in [`GroupSettings`] beside the volume,
//! blend in a snapshot as the volume does, and cross to the audio thread without
//! allocating (`rack.rs`). Master starts with a limiter at −0.3 dBFS
//! ([`EffectChain::master`]); a game removes or retunes it like any other effect.
//!
//! [`GroupSettings`]: super::GroupSettings

mod chain;
pub mod kinds;
mod param;
pub mod rack;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use kira::Frame;
use serde::{Deserialize, Serialize};

pub use chain::{ChainPatch, EffectPatch};
pub use kinds::KINDS;
pub use param::{Fields, Param, Range, Value};

use super::GroupId;

/// The most effects one group runs.
pub const MAX_EFFECTS: usize = 8;
/// The most parameters one effect has (the EQ's six bands of four).
pub const MAX_PARAMS: usize = 24;

/// An effect's kind: its row's index in [`KINDS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KindId(pub u8);

impl KindId {
    /// The row this kind names.
    pub fn row(self) -> &'static EffectKind {
        KINDS[usize::from(self.0)]
    }

    /// The kind called `name`, or an error naming the kinds that exist.
    pub fn named(name: &str) -> Result<KindId, String> {
        let index = KINDS.iter().position(|k| k.name == name).ok_or_else(|| {
            let names: Vec<_> = KINDS.iter().map(|k| k.name).collect();
            format!("no effect kind '{name}' (kinds: {})", names.join(", "))
        })?;
        Ok(KindId(index as u8))
    }
}

/// One effect's settings: its kind, and its values in its row's parameter order
/// (the slots past the row's count stay `0`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectParams {
    pub kind: KindId,
    pub values: [f32; MAX_PARAMS],
}

impl EffectParams {
    /// `kind` with every parameter at its default.
    pub fn defaults(kind: KindId) -> Self {
        let mut values = [0.0; MAX_PARAMS];
        for (v, p) in values.iter_mut().zip(kind.row().params) {
            *v = p.range.default();
        }
        Self { kind, values }
    }

    /// The group a sidechain parameter keys this effect from, if any.
    pub fn key(&self) -> Option<GroupId> {
        let row = self.kind.row();
        let i = row.params.iter().position(|p| p.range == Range::Group)?;
        param::group(self.values[i])
    }

    /// Blend toward `to` by `t` in `[0, 1]`: numbers glide (a log range in
    /// octaves), choices and groups switch to `to`'s at once. A different kind is
    /// not blended: it is `to`.
    pub fn lerp(&self, to: &EffectParams, t: f32) -> EffectParams {
        if self.kind != to.kind {
            return *to;
        }
        let mut out = *to;
        for (i, p) in self.kind.row().params.iter().enumerate() {
            out.values[i] = p.range.lerp(self.values[i], to.values[i], t);
        }
        out
    }
}

/// A group's ordered effects; empty slots sit at the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectChain(pub [Option<EffectParams>; MAX_EFFECTS]);

impl EffectChain {
    /// The effects in order.
    pub fn iter(&self) -> impl Iterator<Item = &EffectParams> {
        self.0.iter().flatten()
    }

    /// A chain of `effects`; more than [`MAX_EFFECTS`] is an error.
    pub fn of(effects: &[EffectParams]) -> Result<EffectChain, String> {
        if effects.len() > MAX_EFFECTS {
            return Err(format!(
                "{} effects on one group (at most {MAX_EFFECTS})",
                effects.len()
            ));
        }
        let mut chain = EffectChain::default();
        for (slot, e) in chain.0.iter_mut().zip(effects) {
            *slot = Some(*e);
        }
        Ok(chain)
    }

    /// Master's default: one limiter at its −0.3 dBFS ceiling, so a loud moment
    /// never takes the mix past it. Master's chain runs after the reverb bus sums
    /// in (`device/groups.rs`, #1855), so the limiter holds the reverb too.
    pub fn master() -> EffectChain {
        let limiter = KindId::named("limiter").map(EffectParams::defaults);
        EffectChain::of(&limiter.into_iter().collect::<Vec<_>>()).unwrap_or_default()
    }

    /// Blend toward `to` by `t`, slot by slot ([`EffectParams::lerp`]).
    pub fn lerp(&self, to: &EffectChain, t: f32) -> EffectChain {
        let mut out = *to;
        for (o, (a, b)) in out.0.iter_mut().zip(self.0.iter().zip(&to.0)) {
            if let (Some(a), Some(b)) = (a, b) {
                *o = Some(a.lerp(b, t));
            }
        }
        out
    }
}

/// One effect kind: a row of [`KINDS`].
pub struct EffectKind {
    /// What a script names it (`kind = "compressor"`).
    pub name: &'static str,
    /// Its parameters, in value order. At most [`MAX_PARAMS`].
    pub params: &'static [Param],
    /// A list field a script writes nested (`bands = { {...}, ... }`): its entries'
    /// fields are the parameters named `<list><n>_<field>`, `n` from 1.
    pub list: Option<&'static str>,
    /// Build the DSP that runs it. Called off the audio thread: it may allocate.
    pub build: fn(&DspContext) -> Box<dyn Dsp>,
}

/// What an effect's DSP is built for.
pub struct DspContext {
    /// The device's sample rate, in Hz.
    pub sample_rate: u32,
    /// The level of the group a sidechain keys from, if any.
    pub key: Option<Arc<KeyLevel>>,
}

/// One effect running on the audio thread. It never allocates or locks.
pub trait Dsp: Send {
    /// Process `block` in place under `values` (its kind's, already glided), at
    /// `dt` seconds per frame.
    fn process(&mut self, values: &[f32; MAX_PARAMS], block: &mut [Frame], dt: f64);

    /// Whether it still holds sound it will play over silent input (a delay's
    /// echo): the device keeps the group awake while it does (`rack.rs`).
    fn ringing(&self) -> bool {
        false
    }
}

/// A group's output peak over its last block, after its volume: what a sidechain
/// keys from.
#[derive(Debug, Default)]
pub struct KeyLevel(AtomicU32);

impl KeyLevel {
    pub fn set(&self, peak: f32) {
        self.0.store(peak.to_bits(), Ordering::Relaxed);
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// A linear gain from decibels.
pub fn gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
#[path = "effects_tests.rs"]
mod effects_tests;
