//! src/audio/mixer/effects/kinds/eq.rs — a six-band parametric EQ (#1748).
//!
//! Godot's `AudioEffectEQ6` made parametric, as Unity's *ParamEQ* is per band: six
//! `bands`, each `{ shape, frequency, gain, q }`. A shape is `off` (the default:
//! the band is skipped, so a fresh EQ is bit-transparent), `peak`, `low_shelf` or
//! `high_shelf`; `frequency` in Hz, `gain` in dB, `q` its width (0.707 is a gentle
//! shelf). Each band is an RBJ-cookbook biquad, re-derived only when its glided
//! parameters move, at most once per rack chunk.

use std::f32::consts::PI;

use kira::Frame;

use super::super::{Dsp, DspContext, EffectKind, Param, Range, MAX_PARAMS};

/// The band shapes, in the order `shape` stores them.
pub const SHAPES: [&str; 4] = ["off", "peak", "low_shelf", "high_shelf"];
/// How many bands an EQ has.
pub const BANDS: usize = 6;

macro_rules! bands {
    ($(($n:literal, $hz:literal)),*) => {
        [$(
            Param { name: concat!("bands", $n, "_shape"), range: Range::Choice { names: &SHAPES, default: 0 } },
            Param { name: concat!("bands", $n, "_frequency"), range: Range::Hz { min: 20.0, max: 22_000.0, default: $hz } },
            Param { name: concat!("bands", $n, "_gain"), range: Range::Number { min: -24.0, max: 24.0, default: 0.0 } },
            Param { name: concat!("bands", $n, "_q"), range: Range::Number { min: 0.1, max: 10.0, default: 0.707 } },
        )*]
    };
}

/// Godot's EQ6 centres as the bands' default frequencies.
const PARAMS: [Param; 4 * BANDS] = bands![
    (1, 32.0),
    (2, 100.0),
    (3, 320.0),
    (4, 1000.0),
    (5, 3200.0),
    (6, 10000.0)
];

pub const KIND: EffectKind = EffectKind {
    name: "eq",
    params: &PARAMS,
    list: Some("bands"),
    build,
};

fn build(_: &DspContext) -> Box<dyn Dsp> {
    Box::new(Eq {
        bands: [Band::default(); BANDS],
    })
}

/// A biquad's normalised coefficients `[b0, b1, b2, a1, a2]`.
pub type Coefficients = [f32; 5];

/// The RBJ coefficients for `shape` (an index of [`SHAPES`], not `off`) at
/// `frequency` Hz, `gain` dB and `q`, at `rate` Hz.
pub fn coefficients(shape: usize, frequency: f32, gain: f32, q: f32, rate: f32) -> Coefficients {
    let w = 2.0 * PI * frequency.min(rate * 0.49) / rate;
    let (sin, cos) = w.sin_cos();
    let a = 10f32.powf(gain / 40.0);
    let alpha = sin / (2.0 * q.max(0.01));
    let s = 2.0 * a.sqrt() * alpha;
    let [b0, b1, b2, a0, a1, a2] = match shape {
        1 => [
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        ],
        2 => [
            a * ((a + 1.0) - (a - 1.0) * cos + s),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - s),
            (a + 1.0) + (a - 1.0) * cos + s,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - s,
        ],
        _ => [
            a * ((a + 1.0) + (a - 1.0) * cos + s),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - s),
            (a + 1.0) - (a - 1.0) * cos + s,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - s,
        ],
    };
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

/// One band: the parameters its coefficients were made from, and its state.
#[derive(Clone, Copy, Default)]
struct Band {
    made: [f32; 5],
    c: Coefficients,
    /// Per channel `[x1, x2, y1, y2]`.
    state: [[f32; 4]; 2],
}

impl Band {
    fn run(&mut self, ch: usize, x: f32) -> f32 {
        let [b0, b1, b2, a1, a2] = self.c;
        let [x1, x2, y1, y2] = self.state[ch];
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        // A NaN would ring in the state forever: the band restarts from silence.
        self.state[ch] = if y.is_finite() {
            [x, x1, y, y1]
        } else {
            [0.0; 4]
        };
        y
    }
}

struct Eq {
    bands: [Band; BANDS],
}

impl Dsp for Eq {
    fn process(&mut self, v: &[f32; MAX_PARAMS], block: &mut [Frame], dt: f64) {
        let rate = (1.0 / dt) as f32;
        for (band, p) in self.bands.iter_mut().zip(v.chunks_exact(4)) {
            let shape = p[0] as usize;
            if shape == 0 {
                band.state = [[0.0; 4]; 2];
                continue;
            }
            let made = [p[0], p[1], p[2], p[3], rate];
            if band.made != made {
                band.made = made;
                band.c = coefficients(shape, p[1], p[2], p[3], rate);
            }
            for f in block.iter_mut() {
                *f = Frame::new(band.run(0, f.left), band.run(1, f.right));
            }
        }
    }
}

#[cfg(test)]
#[path = "eq_tests.rs"]
mod eq_tests;
