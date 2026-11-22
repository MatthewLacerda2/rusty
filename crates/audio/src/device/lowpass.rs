use std::f32::consts::TAU;

use crate::mixer::settings::LOW_PASS_OFF;

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
