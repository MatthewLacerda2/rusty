//! An effect sleeps over silence and never misses a sound (#1729).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::*;

const BUFFER: usize = 128;
const DT: f64 = 1.0 / 48_000.0;
/// Enough buffers of silence to outlast [`HANG`].
const PAST_HANG: usize = (HANG / DT) as usize / BUFFER + 1;

#[derive(Default)]
struct Counts {
    processed: AtomicUsize,
    commands: AtomicUsize,
}

/// An effect that counts its calls and halves its input, plus a tail of `ring` buffers.
struct Probe(Arc<Counts>, usize);

impl Effect for Probe {
    fn on_start_processing(&mut self) {
        self.0.commands.fetch_add(1, Ordering::Relaxed);
    }

    fn process(&mut self, input: &mut [Frame], _dt: f64, _info: &Info) {
        self.0.processed.fetch_add(1, Ordering::Relaxed);
        let ringing = self.1 > 0;
        self.1 = self.1.saturating_sub(1);
        for frame in input {
            *frame = *frame * 0.5 + Frame::from_mono(if ringing { 0.1 } else { 0.0 });
        }
    }
}

struct ProbeBuilder(Arc<Counts>, usize);

impl EffectBuilder for ProbeBuilder {
    type Handle = ();
    fn build(self) -> (Box<dyn Effect>, ()) {
        (Box::new(Probe(self.0, self.1)), ())
    }
}

fn gate(ring: usize) -> (Box<dyn Effect>, Arc<Counts>) {
    let counts = Arc::new(Counts::default());
    let (effect, ()) = Gated(ProbeBuilder(Arc::clone(&counts), ring), Wake::default()).build();
    (effect, counts)
}

/// Run `effect` over `buffers` buffers of a constant `level`; the last buffer out.
fn run(effect: &mut Box<dyn Effect>, level: f32, buffers: usize) -> Vec<Frame> {
    let info = kira::info::MockInfoBuilder::new().build();
    let mut out = Vec::new();
    for _ in 0..buffers {
        out = vec![Frame::from_mono(level); BUFFER];
        effect.on_start_processing();
        effect.process(&mut out, DT, &info);
    }
    out
}

fn processed(counts: &Counts) -> usize {
    counts.processed.load(Ordering::Relaxed)
}

#[test]
fn silence_is_computed_for_the_hang_and_then_not_at_all() {
    let (mut effect, counts) = gate(0);
    run(&mut effect, 0.0, PAST_HANG);
    let slept_at = processed(&counts);
    run(&mut effect, 0.0, 1000);
    assert_eq!(processed(&counts), slept_at, "asleep over silence");
}

#[test]
fn the_first_buffer_of_a_sound_goes_through_the_effect() {
    let (mut effect, counts) = gate(0);
    run(&mut effect, 0.0, PAST_HANG * 2);
    let slept_at = processed(&counts);
    let out = run(&mut effect, 0.5, 1);
    assert_eq!(processed(&counts), slept_at + 1);
    assert_eq!(
        out,
        vec![Frame::from_mono(0.25); BUFFER],
        "first sample included"
    );
}

#[test]
fn a_tail_keeps_it_awake_until_it_dies_away() {
    let (mut effect, counts) = gate(PAST_HANG * 3);
    let out = run(&mut effect, 0.0, PAST_HANG * 3);
    assert_eq!(
        out[0],
        Frame::from_mono(0.1),
        "still ringing over silent input"
    );
    assert_eq!(processed(&counts), PAST_HANG * 3, "awake for all of it");
    run(&mut effect, 0.0, PAST_HANG);
    let slept_at = processed(&counts);
    run(&mut effect, 0.0, 100);
    assert_eq!(processed(&counts), slept_at, "asleep once the tail died");
}

#[test]
fn a_sound_restarts_the_hang() {
    let (mut effect, counts) = gate(0);
    run(&mut effect, 0.0, PAST_HANG);
    run(&mut effect, 0.5, 1);
    let woke_at = processed(&counts);
    run(&mut effect, 0.0, PAST_HANG / 2);
    assert_eq!(
        processed(&counts),
        woke_at + PAST_HANG / 2,
        "not asleep yet"
    );
}

#[test]
fn an_effect_asleep_still_takes_its_commands_and_passes_silence_on() {
    let (mut effect, counts) = gate(0);
    run(&mut effect, 0.0, PAST_HANG);
    let before = counts.commands.load(Ordering::Relaxed);
    let out = run(&mut effect, 0.0, 10);
    assert_eq!(counts.commands.load(Ordering::Relaxed), before + 10);
    assert_eq!(out, vec![Frame::ZERO; BUFFER]);
}

#[test]
fn a_ring_runs_it_over_silence_for_another_hang_and_then_it_sleeps() {
    let wake = Wake::default();
    let counts = Arc::new(Counts::default());
    let builder = Gated(ProbeBuilder(Arc::clone(&counts), 0), wake.clone());
    let (mut effect, ()) = builder.build();
    run(&mut effect, 0.0, PAST_HANG);
    let slept_at = processed(&counts);
    wake.ring();
    run(&mut effect, 0.0, PAST_HANG * 3);
    let woken = processed(&counts) - slept_at;
    assert!(woken >= PAST_HANG - 1, "a tween's worth of frames: {woken}");
    assert!(woken <= PAST_HANG, "and asleep again: {woken}");
}
