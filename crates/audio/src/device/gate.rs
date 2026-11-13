//! crates/audio/src/device/gate.rs — effects that stop computing over silence (#1729).
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
//! The ring is a generation count, not a flag one gate takes, so every gate sharing a
//! [`Wake`] (a group's two filters and its effect rack) sees each ring.
//!
//! An effect whose tail outlasts the hang (a 2 s delay's echo, still in its line while
//! input and output are both silent) holds the gate awake through its [`Hold`] until the
//! tail has played.

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

/// Whether every sample of `frames` is silence.
pub(super) fn is_silent(frames: &[Frame]) -> bool {
    frames
        .iter()
        .all(|f| f.left.abs() < SILENCE && f.right.abs() < SILENCE)
}

/// What a retune rings, after telling the effect's handle: the effect's parameters are
/// about to move, so it stays awake for them. Each ring bumps a generation every gate
/// sharing it compares with the last it saw.
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

/// What an effect with a tail sets while it still has sound to play over silent input;
/// its gate stays awake while it is set.
pub(super) type Hold = Arc<AtomicBool>;

/// Build effect `B` so that it sleeps over silence, woken by the [`Wake`] too, and held
/// awake by its [`Hold`] if it has one. Its handle is `B`'s own.
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

/// An effect, what wakes it, the last ring it saw, what holds it, and how long it has
/// been silent, in seconds.
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

    /// Always, asleep too: this is where an effect takes its handle's latest command.
    fn on_start_processing(&mut self) {
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
