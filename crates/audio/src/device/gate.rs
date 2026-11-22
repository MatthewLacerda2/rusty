use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;

/// Below this a sample is silence: -100 dB, under anything a speaker can show.
const SILENCE: f32 = 1e-5;
/// How long input and output must both stay silent before the effect sleeps, in
/// seconds: longer than a gap between a reverb's echoes.
const HANG: f64 = 1.0;

pub(super) fn is_silent(frames: &[Frame]) -> bool {
    frames
        .iter()
        .all(|f| f.left.abs() < SILENCE && f.right.abs() < SILENCE)
}

/// Rung on a retune: kira moves a parameter only inside `process`, so one taken asleep
/// would still glide when a sound woke it. A count, so every gate sharing it sees each ring.
#[derive(Clone, Default)]
pub(super) struct Wake(Arc<AtomicU64>);

impl Wake {
    pub(super) fn ring(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }

    fn generation(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

pub(super) type Hold = Arc<AtomicBool>;

pub(super) struct Gated<B>(pub B, pub Wake, pub Option<Hold>);

impl<B: EffectBuilder> EffectBuilder for Gated<B> {
    type Handle = B::Handle;

    fn build(self) -> (Box<dyn Effect>, Self::Handle) {
        let (inner, handle) = self.0.build();
        let gate = Gate {
            inner,
            seen: self.1.generation(),
            wake: self.1,
            hold: self.2,
            quiet: 0.0,
        };
        (Box::new(gate), handle)
    }
}

/// `quiet`: seconds of silent input and output with no tail held.
struct Gate {
    inner: Box<dyn Effect>,
    wake: Wake,
    seen: u64,
    hold: Option<Hold>,
    quiet: f64,
}

impl Effect for Gate {
    fn init(&mut self, sample_rate: u32, internal_buffer_size: usize) {
        self.inner.init(sample_rate, internal_buffer_size);
    }

    fn on_change_sample_rate(&mut self, sample_rate: u32) {
        self.inner.on_change_sample_rate(sample_rate);
    }

    fn on_start_processing(&mut self) {
        // Forwarded asleep too: kira's effects take their handle's commands here.
        self.inner.on_start_processing();
        let generation = self.wake.generation();
        if generation != self.seen {
            self.seen = generation;
            self.quiet = 0.0;
        }
    }

    fn process(&mut self, input: &mut [Frame], dt: f64, info: &Info) {
        let silent_in = is_silent(input);
        if silent_in && self.quiet >= HANG {
            return;
        }
        self.inner.process(input, dt, info);
        let held = self
            .hold
            .as_ref()
            .is_some_and(|h| h.load(Ordering::Acquire));
        self.quiet = match silent_in && !held && is_silent(input) {
            true => self.quiet + dt * input.len() as f64,
            false => 0.0,
        };
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod gate_tests;

#[cfg(test)]
#[path = "rack_gate_tests.rs"]
mod rack_gate_tests;
