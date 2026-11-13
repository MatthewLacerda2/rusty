//! crates/audio/src/mixer/effects/kinds/limiter.rs — a brickwall limiter (#1748).
//!
//! Godot's `AudioEffectHardLimiter`: `pre_gain` dB, then no sample past `ceiling`
//! dB. The attack is instant — the gain drops on the very sample that would pass
//! the ceiling, so nothing ever does — and recovers toward unity over `release`
//! ms, landing on it exactly. Below the ceiling, once recovered, it is
//! bit-transparent. A non-finite sample (a NaN out of
//! an effect upstream) is played as silence rather than handed to the device.
//! Master runs one at −0.3 dBFS by default ([`EffectChain::master`]).
//!
//! [`EffectChain::master`]: super::super::EffectChain::master

use kira::Frame;

use super::super::{gain, Dsp, DspContext, EffectKind, Param, Range, MAX_PARAMS};
use super::{peak, smoothing};

const PARAMS: [Param; 3] = [
    Param {
        name: "ceiling",
        range: Range::Number {
            min: -24.0,
            max: 0.0,
            default: -0.3,
        },
    },
    Param {
        name: "pre_gain",
        range: Range::Number {
            min: -24.0,
            max: 24.0,
            default: 0.0,
        },
    },
    Param {
        name: "release",
        range: Range::Number {
            min: 10.0,
            max: 3000.0,
            default: 100.0,
        },
    },
];

pub const KIND: EffectKind = EffectKind {
    name: "limiter",
    params: &PARAMS,
    list: None,
    build,
};

fn build(_: &DspContext) -> Box<dyn Dsp> {
    Box::new(Limiter { gain: 1.0 })
}

struct Limiter {
    /// The gain applied now, in `(0, 1]`.
    gain: f32,
}

impl Dsp for Limiter {
    fn process(&mut self, v: &[f32; MAX_PARAMS], block: &mut [Frame], dt: f64) {
        let [ceiling, pre_gain, release, ..] = *v;
        let (ceiling, pre) = (gain(ceiling), gain(pre_gain));
        let release = smoothing(release, dt);
        for f in block {
            let x = Frame::new(f.left * pre, f.right * pre);
            if !(x.left.is_finite() && x.right.is_finite()) {
                *f = Frame::ZERO;
                continue;
            }
            let level = peak(&x);
            let need = if level > ceiling {
                ceiling / level
            } else {
                1.0
            };
            if need < self.gain {
                self.gain = need;
            } else {
                // A step too small to move an `f32`, or within a hair of the
                // target, lands on it (as `rack::glide` does): a recovered
                // limiter is bit-transparent again.
                let next = self.gain + release * (need - self.gain);
                let settled = next == self.gain || (need - next).abs() <= 1e-6;
                self.gain = if settled { need } else { next };
            }
            *f = if self.gain == 1.0 {
                x
            } else {
                Frame::new(x.left * self.gain, x.right * self.gain)
            };
        }
    }
}

#[cfg(test)]
#[path = "limiter_tests.rs"]
mod limiter_tests;
