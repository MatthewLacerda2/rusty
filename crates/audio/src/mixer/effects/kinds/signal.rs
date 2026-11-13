//! Reference signals for the effect tests (#1748): a sine, its level, and one DSP
//! run over it the way the rack runs it.

use std::f32::consts::TAU;

use kira::Frame;

use super::super::param::{parse, Fields, Value};
use super::super::{DspContext, EffectParams, KindId, MAX_PARAMS};

/// The rate every test runs at.
pub const RATE: u32 = 48_000;
pub const DT: f64 = 1.0 / RATE as f64;

/// `frames` of a sine at `hz` and peak `amp`, the same on both channels.
pub fn sine(hz: f32, amp: f32, frames: usize) -> Vec<Frame> {
    (0..frames)
        .map(|i| {
            let x = amp * (TAU * hz * i as f32 / RATE as f32).sin();
            Frame::new(x, x)
        })
        .collect()
}

/// The left channel's peak over `block`, in dB.
pub fn peak_db(block: &[Frame]) -> f32 {
    let peak = block.iter().fold(0.0f32, |p, f| p.max(f.left.abs()));
    20.0 * peak.max(1e-9).log10()
}

/// `kind`'s values with `fields` (numbers and names) over its defaults.
pub fn values(kind: &str, fields: &[(&str, f64)], names: &[(&str, &str)]) -> [f32; MAX_PARAMS] {
    let kind = KindId::named(kind).expect("kind");
    let mut f: Fields = fields
        .iter()
        .map(|(k, v)| (k.to_string(), Value::Number(*v)))
        .collect();
    f.extend(
        names
            .iter()
            .map(|(k, v)| (k.to_string(), Value::Text(v.to_string()))),
    );
    let base = EffectParams::defaults(kind);
    let none = |n: &str| Err(format!("no group {n}"));
    parse(kind, &f, &base, &none).expect("valid").0.values
}

/// Run `kind` at `values` over `signal` in 128-frame blocks, with `ctx`'s key.
pub fn run_with(kind: &str, values: &[f32; MAX_PARAMS], signal: &mut [Frame], ctx: DspContext) {
    let row = KindId::named(kind).expect("kind").row();
    let mut dsp = (row.build)(&ctx);
    for block in signal.chunks_mut(128) {
        dsp.process(values, block, DT);
    }
}

/// Run `kind` at `values` over `signal`, with no sidechain.
pub fn run(kind: &str, values: &[f32; MAX_PARAMS], signal: &mut [Frame]) {
    let ctx = DspContext {
        sample_rate: RATE,
        key: None,
    };
    run_with(kind, values, signal, ctx);
}
