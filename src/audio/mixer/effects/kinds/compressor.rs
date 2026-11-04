//! src/audio/mixer/effects/kinds/compressor.rs — a feed-forward compressor, with
//! an optional sidechain (#1748).
//!
//! Above `threshold` dB the level is reduced by `ratio` (4:1 lets 1 dB of every 4
//! through); the reduction moves toward its target with `attack` / `release`
//! (ms), measured in dB, and `makeup` dB is added after. The detector is the
//! louder channel's peak (stereo-linked), or with a `sidechain` the keyed group's
//! peak over its last block (Godot's `AudioEffectCompressor.sidechain`): SFX can
//! duck the music under every gunshot. Unity's mixer compressor's names and
//! defaults (`threshold` 0 dB: it does nothing until set), with Godot's `ratio`.

use std::sync::Arc;

use kira::Frame;

use super::super::{gain, Dsp, DspContext, EffectKind, KeyLevel, Param, Range, MAX_PARAMS};
use super::{peak, smoothing};

const PARAMS: [Param; 6] = [
    Param {
        name: "threshold",
        range: Range::Number {
            min: -60.0,
            max: 0.0,
            default: 0.0,
        },
    },
    Param {
        name: "ratio",
        range: Range::Number {
            min: 1.0,
            max: 48.0,
            default: 4.0,
        },
    },
    Param {
        name: "attack",
        range: Range::Number {
            min: 0.1,
            max: 1000.0,
            default: 50.0,
        },
    },
    Param {
        name: "release",
        range: Range::Number {
            min: 1.0,
            max: 5000.0,
            default: 50.0,
        },
    },
    Param {
        name: "makeup",
        range: Range::Number {
            min: 0.0,
            max: 30.0,
            default: 0.0,
        },
    },
    Param {
        name: "sidechain",
        range: Range::Group,
    },
];

pub const KIND: EffectKind = EffectKind {
    name: "compressor",
    params: &PARAMS,
    list: None,
    build,
};

fn build(ctx: &DspContext) -> Box<dyn Dsp> {
    Box::new(Compressor {
        key: ctx.key.clone(),
        reduction: 0.0,
    })
}

/// The gain reduction a level of `level_db` asks for, in dB (`>= 0`).
pub fn reduction(level_db: f32, threshold: f32, ratio: f32) -> f32 {
    let over = level_db - threshold;
    if over > 0.0 {
        over * (1.0 - 1.0 / ratio.max(1.0))
    } else {
        0.0
    }
}

struct Compressor {
    key: Option<Arc<KeyLevel>>,
    /// The gain reduction now, in dB.
    reduction: f32,
}

impl Dsp for Compressor {
    fn process(&mut self, v: &[f32; MAX_PARAMS], block: &mut [Frame], dt: f64) {
        let [threshold, ratio, attack, release, makeup, ..] = *v;
        let (attack, release) = (smoothing(attack, dt), smoothing(release, dt));
        let key = self.key.as_ref().map(|k| k.get());
        for f in block {
            let level = key.unwrap_or_else(|| peak(f));
            // An infinite level would leave the reduction infinite (then NaN) for good.
            let level = if level.is_finite() { level } else { 0.0 };
            let target = reduction(20.0 * level.max(1e-9).log10(), threshold, ratio);
            let k = if target > self.reduction {
                attack
            } else {
                release
            };
            self.reduction += k * (target - self.reduction);
            let db = makeup - self.reduction;
            if db != 0.0 {
                let g = gain(db);
                *f = Frame::new(f.left * g, f.right * g);
            }
        }
    }
}

#[cfg(test)]
#[path = "compressor_tests.rs"]
mod compressor_tests;
