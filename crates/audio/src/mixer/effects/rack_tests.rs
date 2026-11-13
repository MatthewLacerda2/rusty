//! The rack on its own (#1748): an install lands at the next block, a retune
//! glides, a replaced DSP goes back to the game thread, a rate change rebuilds,
//! and a rack's level keys another's compressor.

use kira::effect::Effect;
use kira::info::MockInfoBuilder;
use kira::Frame;

use super::super::kinds::signal::{sine, values, DT, RATE};
use super::super::{gain, EffectChain, EffectParams, KindId};
use super::*;

fn chain(kind: &str, v: [f32; 24]) -> EffectChain {
    let kind = KindId::named(kind).expect("kind");
    EffectChain::of(&[EffectParams { kind, values: v }]).expect("one")
}

/// A built rack and its handle, initialised at 48 kHz.
fn rig() -> (Box<dyn Effect>, RackHandle) {
    let (builder, handle) = rack();
    let (mut effect, ()) = builder.build();
    effect.init(RATE, 128);
    (effect, handle)
}

/// One block of `input` through `effect`.
fn block(effect: &mut Box<dyn Effect>, input: &[Frame]) -> Vec<Frame> {
    let mut out = input.to_vec();
    effect.on_start_processing();
    effect.process(&mut out, DT, &MockInfoBuilder::new().build());
    out
}

const NO_KEY: &dyn Fn(GroupId) -> Option<Arc<KeyLevel>> = &|_| None;

#[test]
fn an_empty_rack_passes_through_and_publishes_its_peak() {
    let (mut rack, handle) = rig();
    let s = sine(1000.0, 0.25, 128);
    assert_eq!(block(&mut rack, &s), s);
    assert!((handle.level().get() - 0.25).abs() < 1e-3);
}

#[test]
fn a_set_chain_runs_from_the_next_block() {
    let (mut rack, mut handle) = rig();
    handle.set(
        &chain("limiter", values("limiter", &[("ceiling", -6.0)], &[])),
        NO_KEY,
    );
    let out = block(&mut rack, &sine(100.0, 1.0, 128));
    assert!(out.iter().all(|f| f.left.abs() <= gain(-6.0) * 1.000_001));
}

#[test]
fn a_retune_glides_and_lands_exactly() {
    let (mut rack, mut handle) = rig();
    let dc = vec![Frame::new(0.5, 0.5); 128];
    let post = |db| {
        chain(
            "distortion",
            values("distortion", &[("post_gain", db)], &[]),
        )
    };
    handle.set(&post(0.0), NO_KEY);
    assert_eq!(block(&mut rack, &dc), dc);
    handle.set(&post(-20.0), NO_KEY);
    let first = block(&mut rack, &dc);
    assert!(first[0].left > 0.4, "no jump: {}", first[0].left);
    assert!(first[127].left < first[0].left, "it moves");
    for _ in 0..100 {
        block(&mut rack, &dc);
    }
    assert_eq!(block(&mut rack, &dc)[0].left, 0.5 * gain(-20.0));
}

#[test]
fn a_replaced_dsp_comes_back_and_the_kind_switch_skips_no_more_than_a_block() {
    let (mut rack, mut handle) = rig();
    handle.set(
        &chain("limiter", values("limiter", &[("ceiling", -6.0)], &[])),
        NO_KEY,
    );
    block(&mut rack, &sine(100.0, 1.0, 128));
    handle.set(
        &chain(
            "distortion",
            values("distortion", &[("post_gain", -6.0)], &[]),
        ),
        NO_KEY,
    );
    let out = block(&mut rack, &[Frame::new(1.0, 1.0); 16]);
    assert!(
        (out[0].left - gain(-6.0)).abs() < 1e-6,
        "the new kind runs at once"
    );
    assert!(handle.retired.try_recv().is_ok(), "the limiter came back");
}

#[test]
fn a_rate_change_rebuilds_every_slot_on_maintain() {
    let (mut rack, mut handle) = rig();
    handle.set(&chain("delay", values("delay", &[], &[])), NO_KEY);
    block(&mut rack, &[Frame::ZERO; 16]);
    rack.on_change_sample_rate(96_000);
    handle.maintain(NO_KEY);
    assert!(!handle.shared.stale.load(Ordering::Relaxed));
    assert_eq!(
        handle
            .send
            .try_send(Install { slot: 7, dsp: None })
            .map_err(|_| ()),
        Ok(()),
        "room"
    );
    block(&mut rack, &[Frame::ZERO; 16]);
    assert!(
        handle.retired.try_recv().is_ok(),
        "the 48 kHz line came back"
    );
}

#[test]
fn one_racks_level_keys_another_racks_compressor() {
    let (mut sfx, sfx_handle) = rig();
    let (mut music, mut handle) = rig();
    let v = values(
        "compressor",
        &[("threshold", -30.0), ("ratio", 10.0), ("attack", 0.1)],
        &[],
    );
    let mut keyed = chain("compressor", v);
    if let Some(e) = keyed.0[0].as_mut() {
        e.values[5] = 1.0; // keyed from group 1
    }
    let level = sfx_handle.level();
    handle.set(&keyed, &|g| (g == GroupId(1)).then(|| Arc::clone(&level)));
    let tone = sine(200.0, 0.1, 128);
    block(&mut sfx, &[Frame::new(1.0, 1.0); 128]);
    let mut out = Vec::new();
    for _ in 0..20 {
        out = block(&mut music, &tone);
    }
    let peak = out.iter().fold(0.0f32, |p, f| p.max(f.left.abs()));
    assert!(peak < 0.01, "the gunshot ducks the music: {peak}");
}
