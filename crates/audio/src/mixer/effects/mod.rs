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

pub const MAX_EFFECTS: usize = 8;
pub const MAX_PARAMS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KindId(pub u8);

impl KindId {
    pub fn row(self) -> &'static EffectKind {
        KINDS[usize::from(self.0)]
    }

    pub fn named(name: &str) -> Result<KindId, String> {
        let index = KINDS.iter().position(|k| k.name == name).ok_or_else(|| {
            let names: Vec<_> = KINDS.iter().map(|k| k.name).collect();
            format!("no effect kind '{name}' (kinds: {})", names.join(", "))
        })?;
        Ok(KindId(index as u8))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectParams {
    pub kind: KindId,
    pub values: [f32; MAX_PARAMS],
}

impl EffectParams {
    pub fn defaults(kind: KindId) -> Self {
        let mut values = [0.0; MAX_PARAMS];
        for (v, p) in values.iter_mut().zip(kind.row().params) {
            *v = p.range.default();
        }
        Self { kind, values }
    }

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectChain(pub [Option<EffectParams>; MAX_EFFECTS]);

impl EffectChain {
    pub fn iter(&self) -> impl Iterator<Item = &EffectParams> {
        self.0.iter().flatten()
    }

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

    pub fn master() -> EffectChain {
        let limiter = KindId::named("limiter").map(EffectParams::defaults);
        EffectChain::of(&limiter.into_iter().collect::<Vec<_>>()).unwrap_or_default()
    }

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

pub struct EffectKind {
    pub name: &'static str,
    pub params: &'static [Param],
    pub list: Option<&'static str>,
    /// Build the DSP that runs it. Called off the audio thread: it may allocate.
    pub build: fn(&DspContext) -> Box<dyn Dsp>,
}

pub struct DspContext {
    /// The device's sample rate, in Hz.
    pub sample_rate: u32,
    pub key: Option<Arc<KeyLevel>>,
}

/// One effect running on the audio thread. It never allocates or locks.
pub trait Dsp: Send {
    /// Process `block` in place under `values` (its kind's, already glided), at
    /// `dt` seconds per frame.
    fn process(&mut self, values: &[f32; MAX_PARAMS], block: &mut [Frame], dt: f64);

    fn ringing(&self) -> bool {
        false
    }
}

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

pub fn gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
#[path = "effects_tests.rs"]
mod effects_tests;
