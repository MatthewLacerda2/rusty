//! src/audio/device/gate.rs — effects that stop computing over silence (#1729).
//!
//! kira runs every effect on every sample, playing or not: with nothing playing, the
//! Freeverb on the reverb bus, each mixer group's two filters and the output stage's
//! compressor still cost about 6 % of a core, forever. [`Gated`] puts an effect to
//! sleep while its input is silent and its tail has died away, as Unity's *Auto Mixer
//! Suspend* does: once the last sound ends it keeps measuring for about a second, and
//! suspends only then.
//!
//! This decides on the device thread from the samples alone, so a sound never waits
//! for a frame or a message: the buffer that first carries it wakes the effect, and its
//! first sample goes through the effect as it always did.
//!
//! A retune rings the effect's [`Wake`]. kira only moves a parameter inside `process`,
//! so a retune taken asleep would still be gliding when the next sound woke the effect:
//! a compressor switched off during silence would clip that sound's start. Woken, the
//! effect runs over silence for another hang, and the tween ends before anything hears it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;

/// Below this a sample is silence: -100 dB, under anything a speaker can show.
const SILENCE: f32 = 1e-5;
/// How long input and output must both stay silent before the effect sleeps, in
/// seconds: longer than a gap between a reverb's echoes.
const HANG: f64 = 1.0;

/// Whether every sample of `frames` is silence.
pub(super) fn is_silent(frames: &[Frame]) -> bool {
    frames
        .iter()
        .all(|f| f.left.abs() < SILENCE && f.right.abs() < SILENCE)
}

/// What a retune rings, after telling the effect's handle: the effect's parameters are
/// about to move, so it stays awake for them.
#[derive(Clone, Default)]
pub(super) struct Wake(Arc<AtomicBool>);

impl Wake {
    pub(super) fn ring(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Build effect `B` so that it sleeps over silence, woken by the [`Wake`] too. Its handle
/// is `B`'s own.
pub(super) struct Gated<B>(pub B, pub Wake);

impl<B: EffectBuilder> EffectBuilder for Gated<B> {
    type Handle = B::Handle;

    fn build(self) -> (Box<dyn Effect>, Self::Handle) {
        let (inner, handle) = self.0.build();
        let gate = Gate {
            inner,
            wake: self.1,
            quiet: 0.0,
        };
        (Box::new(gate), handle)
    }
}

/// An effect, what wakes it, and how long it has been silent, in seconds.
struct Gate {
    inner: Box<dyn Effect>,
    wake: Wake,
    quiet: f64,
}

impl Effect for Gate {
    fn init(&mut self, sample_rate: u32, internal_buffer_size: usize) {
        self.inner.init(sample_rate, internal_buffer_size);
    }

    fn on_change_sample_rate(&mut self, sample_rate: u32) {
        self.inner.on_change_sample_rate(sample_rate);
    }

    /// Always, asleep too: this is where an effect takes its handle's latest command.
    fn on_start_processing(&mut self) {
        self.inner.on_start_processing();
        if self.wake.0.swap(false, Ordering::AcqRel) {
            self.quiet = 0.0;
        }
    }

    fn process(&mut self, input: &mut [Frame], dt: f64, info: &Info) {
        let silent_in = is_silent(input);
        if silent_in && self.quiet >= HANG {
            return;
        }
        self.inner.process(input, dt, info);
        self.quiet = match silent_in && is_silent(input) {
            true => self.quiet + dt * input.len() as f64,
            false => 0.0,
        };
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod gate_tests;
