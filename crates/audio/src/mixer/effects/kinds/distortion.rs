use kira::Frame;

use super::super::{gain, Dsp, DspContext, EffectKind, Param, Range, MAX_PARAMS};

pub const MODES: [&str; 3] = ["soft", "hard", "bitcrush"];
const DRIVE: f32 = 24.0;

const PARAMS: [Param; 4] = [
    Param {
        name: "mode",
        range: Range::Choice {
            names: &MODES,
            default: 0,
        },
    },
    Param {
        name: "drive",
        range: Range::Number {
            min: 0.0,
            max: 1.0,
            default: 0.0,
        },
    },
    Param {
        name: "pre_gain",
        range: Range::Number {
            min: -60.0,
            max: 60.0,
            default: 0.0,
        },
    },
    Param {
        name: "post_gain",
        range: Range::Number {
            min: -80.0,
            max: 24.0,
            default: 0.0,
        },
    },
];

pub const KIND: EffectKind = EffectKind {
    name: "distortion",
    params: &PARAMS,
    list: None,
    build,
};

fn build(_: &DspContext) -> Box<dyn Dsp> {
    Box::new(Distortion)
}

/// One sample through mode `mode` (an index of [`MODES`]) at `drive`.
pub fn shape(mode: usize, drive: f32, x: f32) -> f32 {
    if drive <= 0.0 {
        return x;
    }
    match mode {
        0 => {
            let g = DRIVE * drive;
            (g * x).tanh() / g.tanh()
        }
        1 => (x * (1.0 + DRIVE * drive)).clamp(-1.0, 1.0),
        _ => {
            let steps = (16.0 - 15.0 * drive).exp2() / 2.0;
            (x * steps).round() / steps
        }
    }
}

struct Distortion;

impl Dsp for Distortion {
    fn process(&mut self, v: &[f32; MAX_PARAMS], block: &mut [Frame], _dt: f64) {
        let [mode, drive, pre_gain, post_gain, ..] = *v;
        let (pre, post, mode) = (gain(pre_gain), gain(post_gain), mode as usize);
        for f in block {
            let s = |x: f32| shape(mode, drive, x * pre) * post;
            *f = Frame::new(s(f.left), s(f.right));
        }
    }
}

#[cfg(test)]
#[path = "distortion_tests.rs"]
mod distortion_tests;
