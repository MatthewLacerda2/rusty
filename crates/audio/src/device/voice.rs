use std::sync::atomic::Ordering;
use std::sync::Arc;

use kira::info::Info;
use kira::sound::{Sound, SoundData};
use kira::Frame;

pub use super::control::VoiceControl;
use super::decode::CachedClip;
use super::lowpass::LowPass;

pub fn mix_frame(frame: &[f32], pan: f32, spatial_blend: f32) -> (f32, f32) {
    let (l, r) = match frame {
        [] => (0.0, 0.0),
        [mono] => (*mono, *mono),
        [l, r, ..] => {
            let mono = (l + r) * 0.5;
            (
                l + (mono - l) * spatial_blend,
                r + (mono - r) * spatial_blend,
            )
        }
    };
    let left_gain = (1.0 - pan).min(1.0);
    let right_gain = (1.0 + pan).min(1.0);
    (l * left_gain, r * right_gain)
}

/// Seconds: the time constant a voice's glided parameters, and a rack's, follow with.
pub const SMOOTHING: f64 = 0.010;

/// Within this a glide lands, so a settled voice plays its values bit for bit
/// (−120 dB on a unit gain).
const SNAP: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Glide {
    gain: f32,
    pan: f32,
    blend: f32,
    low_pass: f32,
    doppler: f32,
}

impl Glide {
    fn toward(&mut self, target: &Glide, k: f32) {
        for (v, t) in [
            (&mut self.gain, target.gain),
            (&mut self.pan, target.pan),
            (&mut self.blend, target.blend),
            (&mut self.low_pass, target.low_pass),
            (&mut self.doppler, target.doppler),
        ] {
            let next = *v + k * (t - *v);
            // A step too small to move an `f32` lands too, or it never arrives; so does a
            // non-finite one, so a NaN never sticks past its target.
            *v = if next == *v || !next.is_finite() || (t - next).abs() <= SNAP {
                t
            } else {
                next
            };
        }
    }

    fn filter(&self) -> Option<f32> {
        (self.low_pass < 1.0).then_some(self.low_pass)
    }
}

/// Not a kira sound: those pan constant-power and keep no channel count, which the
/// stereo rule needs.
pub struct ClipSound {
    clip: CachedClip,
    looping: bool,
    control: Arc<VoiceControl>,
    position: f64,
    frame: Vec<f32>,
    filter: LowPass,
    glide: Option<Glide>,
}

impl ClipSound {
    pub fn new(clip: CachedClip, looping: bool, control: Arc<VoiceControl>) -> Self {
        let frame = vec![0.0; usize::from(clip.channels())];
        Self {
            clip,
            looping,
            control,
            position: 0.0,
            frame,
            filter: LowPass::default(),
            glide: None,
        }
    }

    fn frame_at(&self, index: usize) -> &[f32] {
        let len = self.clip.frames();
        let index = if self.looping && len > 0 {
            index % len
        } else {
            index
        };
        self.clip.frame(index)
    }

    fn sample(&mut self) {
        let index = self.position as usize;
        let t = (self.position - index as f64) as f32;
        for c in 0..self.frame.len() {
            let a = self.frame_at(index).get(c).copied().unwrap_or(0.0);
            let b = self.frame_at(index + 1).get(c).copied().unwrap_or(0.0);
            self.frame[c] = a + (b - a) * t;
        }
    }

    fn run_out(&self) -> bool {
        !self.looping && self.position >= self.clip.frames() as f64
    }
}

impl Sound for ClipSound {
    fn process(&mut self, out: &mut [Frame], dt: f64, _info: &Info) {
        let c = &self.control;
        let target = Glide {
            gain: c.gain.get(),
            pan: c.pan.get(),
            blend: c.spatial_blend.get(),
            low_pass: LowPass::coefficient(c.low_pass.get(), dt).unwrap_or(1.0),
            doppler: c.doppler.get(),
        };
        let step = f64::from(c.speed.get()) * f64::from(self.clip.sample_rate()) * dt;
        if c.paused.load(Ordering::Relaxed) || c.stopped.load(Ordering::Relaxed) {
            // Nothing to glide from after silence: a voice retuned while paused resumes
            // on its targets, like a new one.
            self.glide = None;
            out.fill(Frame::ZERO);
            return;
        }
        let k = (1.0 - (-dt / SMOOTHING).exp()) as f32;
        let mut glide = *self.glide.get_or_insert(target);
        for frame in out.iter_mut() {
            if self.run_out() {
                *frame = Frame::ZERO;
                continue;
            }
            glide.toward(&target, k);
            self.sample();
            let mixed = mix_frame(&self.frame, glide.pan, glide.blend);
            let (l, r) = self.filter.process(glide.filter(), mixed);
            *frame = Frame::new(l * glide.gain, r * glide.gain);
            self.position += step * f64::from(glide.doppler);
        }
        self.glide = Some(glide);
        if self.looping && self.clip.frames() > 0 {
            self.position %= self.clip.frames() as f64;
        }
        if self.run_out() {
            self.control.finished.store(true, Ordering::Relaxed);
        }
    }

    fn finished(&self) -> bool {
        !self.control.is_live()
    }
}

pub struct ClipSoundData(pub ClipSound);

impl SoundData for ClipSoundData {
    type Error = ();
    type Handle = ();

    fn into_sound(self) -> Result<(Box<dyn Sound>, ()), ()> {
        Ok((Box::new(self.0), ()))
    }
}

#[cfg(test)]
#[path = "voice_tests.rs"]
mod voice_tests;

#[cfg(test)]
#[path = "glide_tests.rs"]
mod glide_tests;

#[cfg(test)]
#[path = "doppler_glide_tests.rs"]
mod doppler_glide_tests;
