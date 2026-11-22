//! How wide a signal is: how much of it is common to both channels.

use crate::common::{adsr, osc};
use rusty_zimmer::level::{Meter, Profiler};
use rusty_zimmer::patch::{Fx, Patch, Source, Wave};
use rusty_zimmer::{bake_named_note, NoteOpts};

const RATE: f64 = 48_000.0;

fn tone(hz: f64) -> impl Fn(usize) -> f32 {
    move |frame| (std::f64::consts::TAU * hz * frame as f64 / RATE).sin() as f32
}

fn interleaved(
    frames: usize,
    left: impl Fn(usize) -> f32,
    right: impl Fn(usize) -> f32,
) -> Vec<f32> {
    (0..frames).flat_map(|f| [left(f), right(f)]).collect()
}

fn correlation(samples: &[f32], channels: usize) -> Option<f64> {
    let mut meter = Meter::new(channels);
    meter.feed(samples);
    meter.correlation()
}

#[test]
fn the_same_waveform_in_both_ears_is_mono_in_a_stereo_container() {
    let both = tone(440.0);
    let buf = interleaved(48_000, &both, &both);
    let read = correlation(&buf, 2).expect("two channels have a width");
    assert!((read - 1.0).abs() < 1e-9, "identical channels read {read}");
    assert!(read <= 1.0, "and never over the range it is defined on");
}

#[test]
fn a_channel_against_its_own_negation_reads_as_cancelling() {
    let signal = tone(440.0);
    let buf = interleaved(48_000, &signal, |f| -signal(f));
    let read = correlation(&buf, 2).expect("two channels have a width");
    assert!((read + 1.0).abs() < 1e-9, "an inverted channel read {read}");
    assert!(read >= -1.0, "and never under the range it is defined on");
}

/// Over whole periods the partials are orthogonal, so the answer is exactly 1/√2; ±1 come out of
/// many wrong formulas.
#[test]
fn a_signal_half_in_common_reads_as_half_in_common() {
    let fundamental = tone(440.0);
    let octave = tone(880.0);
    let buf = interleaved(48_000, &fundamental, |f| fundamental(f) + octave(f));
    let read = correlation(&buf, 2).expect("two channels have a width");
    let expected = 1.0 / 2.0f64.sqrt();
    assert!(
        (read - expected).abs() < 1e-4,
        "half in common is {expected}, and read {read}"
    );
}

#[test]
fn two_signals_with_nothing_in_common_read_as_nothing_in_common() {
    let buf = interleaved(48_000, tone(440.0), tone(997.0));
    let read = correlation(&buf, 2).expect("two channels have a width");
    assert!(read.abs() < 0.01, "two unrelated tones read {read}");
}

#[test]
fn there_is_no_width_where_there_is_no_pair() {
    let mono: Vec<f32> = (0..48_000).map(tone(440.0)).collect();
    assert_eq!(correlation(&mono, 1), None, "a mono signal has no width");
    let one_sided = interleaved(48_000, tone(440.0), |_| 0.0);
    assert_eq!(
        correlation(&one_sided, 2),
        None,
        "a silent channel has no width to be measured against"
    );
    assert_eq!(
        correlation(&[0.0; 960], 2),
        None,
        "and neither has a silence"
    );
}

#[test]
fn a_row_carries_its_width_beside_its_level() {
    let mut profiler = Profiler::new(2, RATE as u32);
    let both = tone(440.0);
    profiler.feed(&interleaved(48_000, &both, &both));
    let whole = profiler.finish().whole;
    let read = whole.correlation.expect("a stereo row has a width");
    assert!((read - 1.0).abs() < 1e-9, "a centred tone read {read}");

    let mut mono = Profiler::new(1, RATE as u32);
    mono.feed(&(0..48_000).map(tone(440.0)).collect::<Vec<_>>());
    assert_eq!(mono.finish().whole.correlation, None);
}

#[test]
fn a_fully_wet_ensemble_on_a_held_tone_comes_out_cancelling() {
    let patch = Patch {
        layers: Vec::new(),
        source: Source::OscStack {
            oscs: vec![osc(Wave::Sine, 0.0, 0)],
        },
        amp: adsr(0.0, 0.0, 1.0, 0.0),
        filter: None,
        pitch_env: None,
        lfo: None,
        lfo2: None,
        fx: vec![Fx::Chorus {
            rate: 0.3,
            depth: 0.3,
            voices: 2,
            mix: 1.0,
        }],
    };
    let bake = bake_named_note(&patch, "A2", &NoteOpts::default()).expect("a patch bakes");
    let read = bake
        .profile
        .whole
        .correlation
        .expect("a bake is always two channels");
    assert!(read < -0.5, "a cancelling ensemble read {read}");
}
