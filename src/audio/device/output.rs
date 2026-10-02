//! src/audio/device/output.rs — the speaker-mode output stage (#546, #465).
//!
//! The summed mix reaches the device through kira's main track, the one place that
//! sees the *total*, so a footstep and an explosion playing together are processed
//! as one signal. The speaker mode's summed-output half lives there:
//!
//!   * **Home theater** — samples pass through untouched (bit-identical).
//!   * **Headphones** — a light crossfeed ([`CROSSFEED`]) bleeds each channel into
//!     the other, narrowing clips that are wide on their own; per-voice pan
//!     narrowing is `SpeakerMode::shape`.
//!   * **TV** — kira's compressor (downward above −24 dBFS at 4:1, +9 dB makeup
//!     lifting everything below it), then a soft [`limit`] so the lifted mix never
//!     clips.
//!
//! The compressor is kira's, switched fully wet in TV mode and fully dry otherwise;
//! the crossfeed and the limiter are a small [`Shaper`] effect after it, reading the
//! mode from a shared [`OutputControl`] on the audio thread. Platform layer only.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kira::effect::compressor::{CompressorBuilder, CompressorHandle};
use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::track::MainTrackBuilder;
use kira::{Decibels, Frame, Mix, Tween};

use crate::audio::SpeakerMode;

/// How much of each channel headphones mode bleeds into the other.
pub const CROSSFEED: f32 = 0.15;

/// TV compressor: threshold (dBFS), ratio, makeup (dB), attack / release.
const THRESHOLD_DB: f64 = -24.0;
const RATIO: f64 = 4.0;
const MAKEUP_DB: f32 = 9.0;
const ATTACK: Duration = Duration::from_millis(5);
const RELEASE: Duration = Duration::from_millis(200);
/// The soft limiter is linear up to here, then eases toward (never past) 1.0.
const LIMIT_KNEE: f32 = 0.9;

/// The live speaker mode, shared between the backend (writer) and the audio thread.
#[derive(Debug)]
pub struct OutputControl(AtomicU8);

impl OutputControl {
    /// A control starting at `mode`.
    pub fn new(mode: SpeakerMode) -> Arc<Self> {
        let control = Arc::new(Self(AtomicU8::new(0)));
        control.set(mode);
        control
    }

    pub fn set(&self, mode: SpeakerMode) {
        let index = SpeakerMode::ALL.iter().position(|m| *m == mode);
        self.0
            .store(index.unwrap_or_default() as u8, Ordering::Relaxed);
    }

    pub fn get(&self) -> SpeakerMode {
        let index = usize::from(self.0.load(Ordering::Relaxed));
        SpeakerMode::ALL.get(index).copied().unwrap_or_default()
    }
}

/// Soft limiter: identity up to 0.9, then a `tanh` ease that never passes 1.
pub fn limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= LIMIT_KNEE {
        return x;
    }
    let room = 1.0 - LIMIT_KNEE;
    (LIMIT_KNEE + room * ((a - LIMIT_KNEE) / room).tanh()).copysign(x)
}

/// The post-compressor shaping of one summed stereo frame for `mode`.
pub fn shape_frame(mode: SpeakerMode, l: f32, r: f32) -> (f32, f32) {
    match mode {
        SpeakerMode::HomeTheater => (l, r),
        SpeakerMode::Headphones => (
            l * (1.0 - CROSSFEED) + r * CROSSFEED,
            r * (1.0 - CROSSFEED) + l * CROSSFEED,
        ),
        SpeakerMode::Tv => (limit(l), limit(r)),
    }
}

/// The crossfeed / limiter effect (see the module docs).
pub struct Shaper(Arc<OutputControl>);

impl Effect for Shaper {
    fn process(&mut self, input: &mut [Frame], _dt: f64, _info: &Info) {
        let mode = self.0.get();
        if mode == SpeakerMode::HomeTheater {
            return;
        }
        for frame in input {
            let (l, r) = shape_frame(mode, frame.left, frame.right);
            *frame = Frame::new(l, r);
        }
    }
}

impl EffectBuilder for Shaper {
    type Handle = ();
    fn build(self) -> (Box<dyn Effect>, ()) {
        (Box::new(self), ())
    }
}

/// The output stage's controls: the mode the shaper reads and the compressor.
pub struct OutputStage {
    control: Arc<OutputControl>,
    compressor: CompressorHandle,
}

impl OutputStage {
    /// Build the stage onto kira's main track, starting at `mode`.
    pub fn build(main: &mut MainTrackBuilder, mode: SpeakerMode) -> Self {
        let compressor = main.add_effect(
            CompressorBuilder::new()
                .threshold(THRESHOLD_DB)
                .ratio(RATIO)
                .attack_duration(ATTACK)
                .release_duration(RELEASE)
                .makeup_gain(Decibels(MAKEUP_DB))
                .mix(compressor_mix(mode)),
        );
        let control = OutputControl::new(mode);
        main.add_effect(Shaper(Arc::clone(&control)));
        Self {
            control,
            compressor,
        }
    }

    /// Switch the stage to `mode`.
    pub fn set(&mut self, mode: SpeakerMode) {
        self.control.set(mode);
        self.compressor
            .set_mix(compressor_mix(mode), Tween::default());
    }
}

/// Fully wet in TV mode, fully dry (bit-transparent) otherwise.
fn compressor_mix(mode: SpeakerMode) -> Mix {
    if mode == SpeakerMode::Tv {
        Mix::WET
    } else {
        Mix::DRY
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod output_tests;
