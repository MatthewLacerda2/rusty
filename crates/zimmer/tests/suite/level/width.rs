//! How wide a signal is: how much of it is common to both channels.
//!
//! Every buffer here is built so the answer is arithmetic rather than opinion —
//! the same waveform twice, one waveform and its own negation, or two tones
//! that share no period.

use crate::common::{adsr, osc};
use scorsese_zimmer::level::{Meter, Profiler};
use scorsese_zimmer::patch::{Fx, Patch, Source, Wave};
use scorsese_zimmer::{NoteOpts, bake_named_note};

const RATE: f64 = 48_000.0;

/// A tone of `hz`, at `frames` frames, as a function of the frame index.
fn tone(hz: f64) -> impl Fn(usize) -> f32 {
    move |frame| (std::f64::consts::TAU * hz * frame as f64 / RATE).sin() as f32
}

/// An interleaved stereo buffer of `frames` frames from two channel functions.
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

/// The reading a score that never used the `pan` it has comes out at: the same
/// waveform in both ears is `+1`, exactly.
#[test]
fn the_same_waveform_in_both_ears_is_mono_in_a_stereo_container() {
    let both = tone(440.0);
    let buf = interleaved(48_000, &both, &both);
    let read = correlation(&buf, 2).expect("two channels have a width");
    assert!((read - 1.0).abs() < 1e-9, "identical channels read {read}");
    assert!(read <= 1.0, "and never over the range it is defined on");
}

/// **The defect.** A channel against its own negation cancels to nothing the
/// moment anything folds the mix to mono, and this is the one number that says
/// so before somebody plays the video on a phone.
///
/// Constructed rather than baked, because the meter's own claim is about any
/// pair of channels and not about what this crate happens to be able to
/// synthesise. That a recipe *can* now arrive here is the test below.
#[test]
fn a_channel_against_its_own_negation_reads_as_cancelling() {
    let signal = tone(440.0);
    let buf = interleaved(48_000, &signal, |f| -signal(f));
    let read = correlation(&buf, 2).expect("two channels have a width");
    assert!((read + 1.0).abs() < 1e-9, "an inverted channel read {read}");
    assert!(read >= -1.0, "and never under the range it is defined on");
}

/// Half in common, and the arithmetic says exactly how much: the same tone in
/// both ears with an octave added to one side. Over a whole number of periods
/// the two partials are orthogonal, so `Σlr` and `Σl²` are both `N/2` while
/// `Σr²` is `N`, and the answer is `1/√2` and nothing else.
///
/// The two readings either end of the range are both fixed points of the
/// arithmetic — `+1` and `-1` come back out of a great many wrong formulas,
/// and the clamp hands them back besides. A value in between is what says the
/// normalisation is the one it claims to be.
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

/// Two tones that share no period have nothing in common, which is what `0`
/// means — neither the same signal nor the opposite of it.
#[test]
fn two_signals_with_nothing_in_common_read_as_nothing_in_common() {
    let buf = interleaved(48_000, tone(440.0), tone(997.0));
    let read = correlation(&buf, 2).expect("two channels have a width");
    assert!(read.abs() < 0.01, "two unrelated tones read {read}");
}

/// A number about a *pair* of channels is not a number a single channel has,
/// and a silent side is a division by zero rather than a zero.
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

/// The figure travels with the row it belongs to, which is the whole reason it
/// lives on a span: a section that is wide and a section that is mono are two
/// different rows of the same table.
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

/// **A recipe can reach the defect**, which is the half of this that was not
/// true when the figure was first proposed and deferred: back then every source
/// was one waveform in both channels or two independent draws, the pan gains
/// were clamped at zero, and nothing inverted a polarity — so a negative
/// reading was unreachable and the column could never have moved.
///
/// The ensemble changed that. It is two delayed copies of a mono sum panned
/// hard apart, and on a held tone the two sides sit far enough out of step to
/// cancel: fully wet, this one reads about −0.91 and would vanish on anything
/// that sums it to mono. Nothing refuses it, and nothing should — but a report
/// that could not say it was the one defect a mix can have and hide.
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
