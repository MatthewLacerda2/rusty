//! The gate around a group's effects (#1729, #1748): every gate sharing a wake
//! wakes on one ring, a delay's echo holds its gate awake until it has played, and
//! a muted group publishes no level for a sidechain.

use std::sync::Arc;

use kira::effect::{Effect, EffectBuilder};
use kira::info::MockInfoBuilder;
use kira::Frame;

use super::{Gated, Wake, HANG};
use crate::mixer::effects::rack::{rack, RackHandle};
use crate::mixer::effects::{EffectChain, EffectParams, KeyLevel, KindId};
use crate::mixer::GroupId;

const RATE: u32 = 48_000;
const DT: f64 = 1.0 / RATE as f64;
const BLOCK: usize = 128;
const NO_KEY: &dyn Fn(GroupId) -> Option<Arc<KeyLevel>> = &|_| None;

/// A group's rack behind its gate, sharing `wake`, held by its tail.
fn gated_rack(wake: &Wake) -> (Box<dyn Effect>, RackHandle) {
    let (builder, handle) = rack();
    let (mut effect, ()) = Gated(builder, wake.clone(), Some(handle.tail())).build();
    effect.init(RATE, BLOCK);
    (effect, handle)
}

/// `seconds` of `level` through `effect`, block by block; what came out.
fn run(effect: &mut Box<dyn Effect>, level: f32, seconds: f64) -> Vec<Frame> {
    let info = MockInfoBuilder::new().build();
    let blocks = (seconds * f64::from(RATE)).round() as usize / BLOCK;
    let mut out = Vec::new();
    for _ in 0..blocks {
        let mut block = vec![Frame::from_mono(level); BLOCK];
        effect.on_start_processing();
        effect.process(&mut block, DT, &info);
        out.extend(block);
    }
    out
}

fn peak(frames: &[Frame]) -> f32 {
    frames.iter().fold(0.0, |p, f| p.max(f.left.abs()))
}

/// A 2 s delay that echoes once, at full level.
fn slapback() -> EffectChain {
    let kind = KindId::named("delay").expect("delay");
    let mut delay = EffectParams::defaults(kind);
    delay.values[..2].copy_from_slice(&[2000.0, 0.0]); // delay ms, decay
    EffectChain::of(&[delay]).expect("one")
}

#[test]
fn one_ring_wakes_every_gate_that_shares_it() {
    let wake = Wake::default();
    let (mut a, ha) = gated_rack(&wake);
    let (mut b, hb) = gated_rack(&wake);
    run(&mut a, 0.0, HANG + 0.1);
    run(&mut b, 0.0, HANG + 0.1);
    wake.ring();
    // Awake, an empty rack publishes this block's peak; asleep, it keeps the last.
    let (ha_level, hb_level) = (ha.level(), hb.level());
    ha_level.set(9.0);
    hb_level.set(9.0);
    run(&mut a, 0.0, 0.01);
    run(&mut b, 0.0, 0.01);
    assert_eq!(ha_level.get(), 0.0, "the first gate woke");
    assert_eq!(hb_level.get(), 0.0, "and so did the second");
}

#[test]
fn a_delay_echo_plays_after_a_silence_and_not_into_the_next_sound() {
    let (mut fx, mut handle) = gated_rack(&Wake::default());
    handle.set(&slapback(), NO_KEY);
    let shot = run(&mut fx, 0.5, 0.1);
    assert_eq!(peak(&shot), 0.5, "the shot, dry");
    let after = run(&mut fx, 0.0, 2.9);
    let echo = &after[(1.9 * f64::from(RATE)) as usize..(2.0 * f64::from(RATE)) as usize];
    assert!(peak(echo) > 0.49, "the echo at 2 s: {}", peak(echo));
    run(&mut fx, 0.0, 3.0); // the line empties, the gate sleeps
    let next = run(&mut fx, 0.5, 0.1);
    assert!(
        next.iter().all(|f| f.left == 0.5),
        "no stale echo in the shot"
    );
    let tail = run(&mut fx, 0.0, 1.8);
    assert_eq!(peak(&tail), 0.0, "nothing until its own echo");
}

#[test]
fn a_muted_group_keys_nothing() {
    let (mut fx, handle) = gated_rack(&Wake::default());
    run(&mut fx, 0.5, 0.01);
    assert!((handle.level().get() - 0.5).abs() < 1e-6);
    handle.set_output_gain(0.0);
    run(&mut fx, 0.5, 0.01);
    assert_eq!(handle.level().get(), 0.0, "muted: no level");
    handle.set_output_gain(0.5);
    run(&mut fx, 0.5, 0.01);
    assert!(
        (handle.level().get() - 0.25).abs() < 1e-6,
        "after the volume"
    );
}
