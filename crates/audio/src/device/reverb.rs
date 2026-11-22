use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kira::backend::Backend;
use kira::effect::reverb::{ReverbBuilder, ReverbHandle};
use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::track::{SendTrackBuilder, SendTrackHandle, SendTrackId};
use kira::{AudioManager, Frame, Mix, Tween};

use super::gate::{is_silent, Gated, Wake};
use super::groups::decibels;
use rusty_components::audio::reverb_zone::MAX_PRE_DELAY;
use rusty_components::ReverbParams;

/// The mean delay of Freeverb's comb filters at its 44.1 kHz reference (1116–1617
/// samples), in seconds: one trip round the feedback loop.
const COMB_LOOP: f32 = 1356.0 / 44_100.0;
/// The highest feedback the decay maps to; 1 would ring forever.
const MAX_FEEDBACK: f32 = 0.98;
const RETUNE: Duration = Duration::from_millis(50);

/// The comb feedback whose tail falls 60 dB in `decay_time` seconds: `g` per loop,
/// `g^(T / loop) = 10^-3`.
pub fn feedback(decay_time: f32) -> f32 {
    let g = 10f32.powf(-3.0 * COMB_LOOP / decay_time.max(0.01));
    g.min(MAX_FEEDBACK)
}

pub struct ReverbBus {
    track: SendTrackHandle,
    reverb: ReverbHandle,
    pre_delay: Arc<AtomicU32>,
    wake: Wake,
}

impl ReverbBus {
    pub fn new<B: Backend>(manager: &mut AudioManager<B>, params: &ReverbParams) -> Option<Self> {
        let pre_delay = Arc::new(AtomicU32::new(params.pre_delay.to_bits()));
        let mut builder = SendTrackBuilder::new().volume(decibels(params.wet));
        builder.add_effect(PreDelay::new(Arc::clone(&pre_delay)));
        let wake = Wake::default();
        let reverb = builder.add_effect(Gated(
            ReverbBuilder::new()
                .feedback(f64::from(feedback(params.decay_time)))
                .damping(f64::from(params.damping))
                .mix(Mix::WET),
            wake.clone(),
            None,
        ));
        Some(Self {
            track: manager.add_send_track(builder).ok()?,
            reverb,
            pre_delay,
            wake,
        })
    }

    pub fn id(&self) -> SendTrackId {
        self.track.id()
    }

    pub fn set(&mut self, params: &ReverbParams) {
        let tween = Tween {
            duration: RETUNE,
            ..Tween::default()
        };
        self.track.set_volume(decibels(params.wet), tween);
        let feedback = f64::from(feedback(params.decay_time));
        self.reverb.set_feedback(feedback, tween);
        self.reverb.set_damping(f64::from(params.damping), tween);
        self.wake.ring();
        self.pre_delay
            .store(params.pre_delay.to_bits(), Ordering::Relaxed);
    }
}

struct PreDelay {
    target: Arc<AtomicU32>,
    buffer: Vec<Frame>,
    write: usize,
    /// The current delay, in samples.
    delay: f32,
    sample_rate: f32,
    quiet: usize,
}

impl PreDelay {
    fn new(target: Arc<AtomicU32>) -> Self {
        Self {
            target,
            buffer: Vec::new(),
            write: 0,
            delay: 0.0,
            sample_rate: 0.0,
            quiet: 0,
        }
    }

    /// The target delay in samples, inside the line.
    fn target_samples(&self) -> f32 {
        let seconds = f32::from_bits(self.target.load(Ordering::Relaxed));
        let max = (self.buffer.len() - 2) as f32;
        (seconds * self.sample_rate).clamp(0.0, max)
    }

    /// The line `delay` samples behind the write head, linearly interpolated.
    fn read(&self, delay: f32) -> Frame {
        let len = self.buffer.len();
        let back = delay.floor() as usize;
        let frac = delay - back as f32;
        let at = |n: usize| self.buffer[(self.write + len - n) % len];
        at(back) * (1.0 - frac) + at(back + 1) * frac
    }
}

impl Effect for PreDelay {
    fn init(&mut self, sample_rate: u32, _internal_buffer_size: usize) {
        self.sample_rate = sample_rate as f32;
        let len = (MAX_PRE_DELAY * self.sample_rate).ceil() as usize + 2;
        self.buffer = vec![Frame::ZERO; len];
        self.write = 0;
        self.delay = self.target_samples();
        self.quiet = 0;
    }

    fn on_change_sample_rate(&mut self, sample_rate: u32) {
        self.init(sample_rate, 0);
    }

    fn process(&mut self, input: &mut [Frame], _dt: f64, _info: &Info) {
        let target = self.target_samples();
        let silent = is_silent(input);
        if self.quiet >= self.buffer.len() {
            self.delay = target;
            if silent {
                return;
            }
        }
        self.quiet = if silent { self.quiet + input.len() } else { 0 };
        for frame in input {
            self.buffer[self.write] = *frame;
            self.delay += (target - self.delay).clamp(-0.5, 0.5);
            *frame = self.read(self.delay);
            self.write = (self.write + 1) % self.buffer.len();
        }
    }
}

impl EffectBuilder for PreDelay {
    type Handle = ();
    fn build(self) -> (Box<dyn Effect>, ()) {
        (Box::new(self), ())
    }
}

#[cfg(test)]
#[path = "reverb_tests.rs"]
mod reverb_tests;
