//! src/audio/mixer/effects/kinds/delay.rs — an echo, with a low-pass on its
//! feedback (#1748).
//!
//! Unity's mixer *Echo*: the input comes back `delay` ms later at `wet`, and each
//! echo feeds the next at `decay` (`0..1`); the input itself plays at `dry`.
//! Godot's feedback low-pass rides on top: `low_pass` Hz darkens every repeat
//! (22 kHz is open), as a slapback off a far wall loses its top.
//!
//! The line is sized for the longest `delay` ([`MAX_SECONDS`]) at the device's
//! rate when the effect is built, off the audio thread; nothing resizes it while
//! playing, so a snapshot can glide the time anywhere in range. A device whose rate
//! changes rebuilds it (`rack.rs`); until then a delay past the line's end is
//! held at its end.

use std::f32::consts::TAU;

use kira::Frame;

use super::super::{Dsp, DspContext, EffectKind, Param, Range, MAX_PARAMS};

/// The longest delay, in seconds (Unity's echo: 5000 ms).
pub const MAX_SECONDS: f32 = 5.0;
/// A feedback low-pass at or above this is open.
const OPEN: f32 = 22_000.0;
/// Below this a sample written to the line is silence (the device gate's −100 dB).
const SILENT: f32 = 1e-5;

const PARAMS: [Param; 5] = [
    Param {
        name: "delay",
        range: Range::Number {
            min: 10.0,
            max: MAX_SECONDS * 1000.0,
            default: 500.0,
        },
    },
    Param {
        name: "decay",
        range: Range::Number {
            min: 0.0,
            max: 1.0,
            default: 0.5,
        },
    },
    Param {
        name: "dry",
        range: Range::Number {
            min: 0.0,
            max: 1.0,
            default: 1.0,
        },
    },
    Param {
        name: "wet",
        range: Range::Number {
            min: 0.0,
            max: 1.0,
            default: 1.0,
        },
    },
    Param {
        name: "low_pass",
        range: Range::Hz {
            min: 20.0,
            max: OPEN,
            default: OPEN,
        },
    },
];

pub const KIND: EffectKind = EffectKind {
    name: "delay",
    params: &PARAMS,
    list: None,
    build,
};

fn build(ctx: &DspContext) -> Box<dyn Dsp> {
    let frames = (MAX_SECONDS * ctx.sample_rate as f32).ceil() as usize + 2;
    Box::new(Delay {
        line: vec![Frame::ZERO; frames],
        write: 0,
        low: Frame::ZERO,
        quiet: frames,
    })
}

struct Delay {
    line: Vec<Frame>,
    write: usize,
    /// The feedback low-pass's state.
    low: Frame,
    /// Frames written since the last one above [`SILENT`]: once it reaches the
    /// line's length, nothing in the line is left to echo.
    quiet: usize,
}

impl Delay {
    /// The line `back` frames (fractional) behind the write head.
    fn read(&self, back: f32) -> Frame {
        let len = self.line.len();
        let whole = back as usize;
        let t = back - whole as f32;
        let at = |n: usize| self.line[(self.write + len - n % len) % len];
        let (a, b) = (at(whole), at(whole + 1));
        Frame::new(
            a.left + (b.left - a.left) * t,
            a.right + (b.right - a.right) * t,
        )
    }
}

impl Dsp for Delay {
    fn process(&mut self, v: &[f32; MAX_PARAMS], block: &mut [Frame], dt: f64) {
        let [delay, decay, dry, wet, low_pass, ..] = *v;
        let max = (self.line.len() - 2) as f32;
        let back = ((f64::from(delay) / 1000.0 / dt) as f32).clamp(1.0, max);
        let a = (low_pass < OPEN).then(|| 1.0 - (-TAU * low_pass * dt as f32).exp());
        for f in block {
            let echo = self.read(back);
            let fed = match a {
                Some(a) => {
                    self.low.left += a * (echo.left - self.low.left);
                    self.low.right += a * (echo.right - self.low.right);
                    self.low
                }
                None => {
                    self.low = echo;
                    echo
                }
            };
            // A NaN would ring in the line forever: it is fed in as silence.
            let finite = f.left.is_finite() && f.right.is_finite();
            let input = if finite { *f } else { Frame::ZERO };
            let written = Frame::new(
                input.left + decay * fed.left,
                input.right + decay * fed.right,
            );
            self.quiet = match written.left.abs().max(written.right.abs()) < SILENT {
                true => self.quiet.saturating_add(1),
                false => 0,
            };
            self.line[self.write] = written;
            self.write = (self.write + 1) % self.line.len();
            *f = Frame::new(
                dry * input.left + wet * echo.left,
                dry * input.right + wet * echo.right,
            );
        }
    }

    fn ringing(&self) -> bool {
        self.quiet < self.line.len()
    }
}

#[cfg(test)]
#[path = "delay_tests.rs"]
mod delay_tests;
