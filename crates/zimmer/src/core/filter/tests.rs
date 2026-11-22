//! The filter's modes, slopes and stopbands, measured on sines.

use super::*;
use crate::patch::{Adsr, Slope};
use std::f64::consts::TAU;

fn filter(kind: FilterKind, cutoff: f32) -> Filter {
    Filter {
        kind,
        cutoff,
        slope: Slope::Db12,
        resonance: 0.0,
        env_octaves: 0.0,
        vel_octaves: 0.0,
        key_track: 0.0,
        adsr: Adsr::default(),
    }
}

// Phase in `f64`: `f32` rounding a second in leaves a floor near −72 dB, above the −80 a
// stopband test asks.
fn sine(hz: f32) -> Vec<f32> {
    (0..44_100)
        .map(|i| (TAU * f64::from(hz) * f64::from(i) / 44_100.0).sin() as f32)
        .collect()
}

fn settled_peak(buf: &[f32]) -> f32 {
    buf[buf.len() / 2..]
        .iter()
        .fold(0.0f32, |m, s| m.max(s.abs()))
}

#[test]
fn lowpass_passes_below_and_stops_above_the_cutoff() {
    let cutoffs = vec![1000.0; 44_100];
    let mut low = sine(100.0);
    let mut high = sine(8000.0);
    apply(
        &mut low,
        &filter(FilterKind::Lowpass, 1000.0),
        &cutoffs,
        44_100.0,
    );
    apply(
        &mut high,
        &filter(FilterKind::Lowpass, 1000.0),
        &cutoffs,
        44_100.0,
    );
    assert!(settled_peak(&low) > 0.7, "100 Hz survives a 1 kHz lowpass");
    assert!(settled_peak(&high) < 0.1, "8 kHz does not");
}

#[test]
fn highpass_is_the_mirror_image() {
    let cutoffs = vec![2000.0; 44_100];
    let mut low = sine(100.0);
    let mut high = sine(8000.0);
    apply(
        &mut low,
        &filter(FilterKind::Highpass, 2000.0),
        &cutoffs,
        44_100.0,
    );
    apply(
        &mut high,
        &filter(FilterKind::Highpass, 2000.0),
        &cutoffs,
        44_100.0,
    );
    assert!(settled_peak(&low) < 0.1, "100 Hz is cut");
    assert!(settled_peak(&high) > 0.7, "8 kHz passes");
}

#[test]
fn a_steep_highpass_holds_a_low_tone_far_down() {
    let cutoffs = vec![9000.0; 44_100];
    let mut low = sine(560.0);
    apply(
        &mut low,
        &Filter {
            slope: Slope::Db24,
            ..filter(FilterKind::Highpass, 9000.0)
        },
        &cutoffs,
        44_100.0,
    );
    // Four octaves under 9 kHz at 24 dB is −96 dB in theory; the bound catches the 2× image folding
    // back.
    let db = 20.0 * settled_peak(&low).log10();
    assert!(
        db < -80.0,
        "560 Hz leaks through a 9 kHz highpass at {db:.1} dB"
    );
}

#[test]
fn bandpass_keeps_the_middle_and_drops_both_ends() {
    let cutoffs = vec![1000.0; 44_100];
    let band = filter(FilterKind::Bandpass, 1000.0);
    let mut low = sine(60.0);
    let mut mid = sine(1000.0);
    let mut high = sine(9000.0);
    for buf in [&mut low, &mut mid, &mut high] {
        apply(buf, &band, &cutoffs, 44_100.0);
    }
    let centre = settled_peak(&mid);
    assert!(centre > settled_peak(&low) * 4.0, "60 Hz is off the bottom");
    assert!(centre > settled_peak(&high) * 4.0, "9 kHz is off the top");
}

#[test]
fn notch_is_the_bandpass_turned_inside_out() {
    let cutoffs = vec![1000.0; 44_100];
    let notch = filter(FilterKind::Notch, 1000.0);
    let mut low = sine(60.0);
    let mut mid = sine(1000.0);
    for buf in [&mut low, &mut mid] {
        apply(buf, &notch, &cutoffs, 44_100.0);
    }
    assert!(settled_peak(&low) > 0.7, "60 Hz passes a 1 kHz notch");
    assert!(
        settled_peak(&mid) < settled_peak(&low) * 0.5,
        "1 kHz does not"
    );
}

#[test]
fn two_pole_pairs_reject_further_past_the_cutoff_than_one() {
    let cutoffs = vec![1000.0; 44_100];
    let mut gentle = sine(4000.0);
    let mut steep = sine(4000.0);
    apply(
        &mut gentle,
        &filter(FilterKind::Lowpass, 1000.0),
        &cutoffs,
        44_100.0,
    );
    apply(
        &mut steep,
        &Filter {
            slope: Slope::Db24,
            ..filter(FilterKind::Lowpass, 1000.0)
        },
        &cutoffs,
        44_100.0,
    );
    assert!(
        settled_peak(&steep) < settled_peak(&gentle) * 0.5,
        "24 dB rejects harder than 12: {} vs {}",
        settled_peak(&gentle),
        settled_peak(&steep)
    );
}

#[test]
fn resonance_is_a_stronger_control_at_the_steeper_slope() {
    let cutoffs = vec![1000.0; 44_100];
    let mut gentle = sine(1000.0);
    let mut steep = sine(1000.0);
    let mut f = filter(FilterKind::Lowpass, 1000.0);
    f.resonance = 0.9;
    apply(&mut gentle, &f, &cutoffs, 44_100.0);
    f.slope = Slope::Db24;
    apply(&mut steep, &f, &cutoffs, 44_100.0);
    assert!(
        settled_peak(&steep) > settled_peak(&gentle),
        "the peak is applied twice"
    );
}

#[test]
fn cascading_at_the_ceiling_stays_finite() {
    // 14_700 Hz is the ceiling, a third of the rate.
    for cutoff in [20.0, 14_700.0, 1e9] {
        let mut buf = sine(440.0);
        let cutoffs = vec![cutoff; buf.len()];
        let mut f = filter(FilterKind::Lowpass, cutoff);
        f.slope = Slope::Db24;
        f.resonance = 0.99;
        apply(&mut buf, &f, &cutoffs, 44_100.0);
        assert!(
            buf.iter().all(|s| s.is_finite()),
            "cutoff {cutoff} produced non-finite samples at 24 dB"
        );
        assert!(
            buf.iter().all(|s| s.abs() < 100.0),
            "cutoff {cutoff} rang away rather than settling"
        );
    }
}

#[test]
fn resonance_lifts_the_response_at_the_cutoff() {
    let cutoffs = vec![1000.0; 44_100];
    let mut flat = sine(1000.0);
    let mut resonant = sine(1000.0);
    let mut f = filter(FilterKind::Lowpass, 1000.0);
    apply(&mut flat, &f, &cutoffs, 44_100.0);
    f.resonance = 0.9;
    apply(&mut resonant, &f, &cutoffs, 44_100.0);
    assert!(settled_peak(&resonant) > settled_peak(&flat));
}

#[test]
fn extreme_cutoffs_stay_finite_and_bounded() {
    for cutoff in [0.0, -500.0, 1e9, f32::INFINITY] {
        let mut buf = sine(440.0);
        let cutoffs = vec![cutoff; buf.len()];
        let mut f = filter(FilterKind::Lowpass, 1000.0);
        f.resonance = 0.99;
        apply(&mut buf, &f, &cutoffs, 44_100.0);
        assert!(
            buf.iter().all(|s| s.is_finite()),
            "cutoff {cutoff} produced non-finite samples"
        );
    }
}

#[test]
fn a_poisoned_sample_resets_the_filter_instead_of_spreading() {
    // One non-finite input would otherwise infect the integrators forever, so every later sample
    // would come out NaN. The guard flushes the state.
    let mut buf = sine(440.0);
    buf[100] = f32::INFINITY;
    buf[200] = f32::NAN;
    let cutoffs = vec![1000.0; buf.len()];
    apply(
        &mut buf,
        &filter(FilterKind::Lowpass, 1000.0),
        &cutoffs,
        44_100.0,
    );
    assert!(
        buf.iter().all(|s| s.is_finite()),
        "the poison was contained"
    );
    assert!(
        settled_peak(&buf) > 0.5,
        "it recovered and kept passing signal"
    );
}
