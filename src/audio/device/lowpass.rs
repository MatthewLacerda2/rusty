//! src/audio/device/lowpass.rs — a voice's own low-pass filter (#467).
//!
//! Occlusion muffles one voice, not its whole group, so the filter lives in the
//! voice rather than on a kira track. Two cascaded one-pole stages per channel
//! (12 dB/octave): cheap enough for hundreds of voices and steep enough that a
//! wall reads as a wall. At [`LOW_PASS_OFF`] the filter is bypassed, so an
//! unoccluded voice stays bit-identical to its dry signal; while bypassed the
//! state follows the input, so closing the filter later never clicks.

use std::f32::consts::TAU;

use crate::audio::mixer::settings::LOW_PASS_OFF;

/// One voice's stereo low-pass: `[stage][channel]` state.
#[derive(Debug, Default)]
pub struct LowPass {
    state: [[f32; 2]; 2],
}

impl LowPass {
    /// The one-pole smoothing coefficient for `cutoff` Hz at `dt` seconds per
    /// output frame; `None` when the filter is open (bypassed).
    pub fn coefficient(cutoff: f32, dt: f64) -> Option<f32> {
        if cutoff >= LOW_PASS_OFF {
            return None;
        }
        Some(1.0 - (-TAU * cutoff.max(1.0) * dt as f32).exp())
    }

    /// Filter one stereo frame with coefficient `a` (`None`: pass it through).
    pub fn process(&mut self, a: Option<f32>, frame: (f32, f32)) -> (f32, f32) {
        let input = [frame.0, frame.1];
        let Some(a) = a else {
            self.state = [input; 2];
            return frame;
        };
        for (c, x) in input.into_iter().enumerate() {
            let first = &mut self.state[0][c];
            *first += a * (x - *first);
            let y = *first;
            let second = &mut self.state[1][c];
            *second += a * (y - *second);
        }
        (self.state[1][0], self.state[1][1])
    }
}

#[cfg(test)]
#[path = "lowpass_tests.rs"]
mod lowpass_tests;
