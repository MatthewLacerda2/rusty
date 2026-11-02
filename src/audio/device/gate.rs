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
//! first sample goes through the effect as it always did. A retune that arrives while
//! asleep is still taken ([`Effect::on_start_processing`] always runs); its tween starts
//! when the effect next wakes.

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

/// Build effect `B` so that it sleeps over silence. Its handle is `B`'s own.
pub(super) struct Gated<B>(pub B);

impl<B: EffectBuilder> EffectBuilder for Gated<B> {
    type Handle = B::Handle;

    fn build(self) -> (Box<dyn Effect>, Self::Handle) {
        let (inner, handle) = self.0.build();
        let gate = Gate { inner, quiet: 0.0 };
        (Box::new(gate), handle)
    }
}

/// An effect and how long it has been silent, in seconds.
struct Gate {
    inner: Box<dyn Effect>,
    quiet: f64,
}

impl Effect for Gate {
    fn init(&mut self, sample_rate: u32, internal_buffer_size: usize) {
        self.inner.init(sample_rate, internal_buffer_size);
    }

    fn on_change_sample_rate(&mut self, sample_rate: u32) {
        self.inner.on_change_sample_rate(sample_rate);
    }

    /// Always, asleep too: this is where an effect takes its handle's commands, and
    /// their queue would fill up otherwise.
    fn on_start_processing(&mut self) {
        self.inner.on_start_processing();
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
