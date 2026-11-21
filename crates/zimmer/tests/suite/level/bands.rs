//! Where the energy sits, on signals whose spectrum is one frequency.

use std::f32::consts::TAU;

use scorsese_zimmer::level::BandMeter;

const RATE: u32 = 48_000;

fn tone(hz: f32) -> Vec<f32> {
    (0..RATE)
        .map(|i| (TAU * hz * i as f32 / RATE as f32).sin() * 0.8)
        .collect()
}

fn shares(samples: &[f32]) -> (f64, f64, f64) {
    let mut meter = BandMeter::new(1, RATE);
    meter.feed(samples);
    let bands = meter.finish().expect("audible");
    (bands.low, bands.mid, bands.high)
}

#[test]
fn a_low_tone_reports_its_energy_in_the_low_band() {
    let (low, mid, high) = shares(&tone(60.0));
    assert!(low > 0.9, "60 Hz is bass: low {low:.3}, mid {mid:.3}");
    assert!(high < 0.01, "and nothing up top: high {high:.3}");
}

#[test]
fn a_midrange_tone_reports_its_energy_in_the_mid_band() {
    let (low, mid, high) = shares(&tone(1_000.0));
    assert!(mid > 0.95, "1 kHz is midrange: mid {mid:.3}, low {low:.3}");
    assert!(high < 0.02, "high {high:.3}");
}

#[test]
fn a_high_tone_reports_its_energy_in_the_high_band() {
    let (low, mid, high) = shares(&tone(12_000.0));
    assert!(
        high > 0.75,
        "12 kHz is up top: high {high:.3}, mid {mid:.3}"
    );
    assert!(high > mid + low, "and it is where most of the energy is");
}

#[test]
fn the_three_shares_are_shares_of_one_whole() {
    for hz in [60.0, 400.0, 3_000.0, 9_000.0] {
        let (low, mid, high) = shares(&tone(hz));
        assert!(
            (low + mid + high - 1.0).abs() < 1e-9,
            "{hz} Hz: {low} + {mid} + {high}"
        );
    }
}

#[test]
fn a_silence_has_no_balance_at_all() {
    let mut meter = BandMeter::new(1, RATE);
    meter.feed(&[0.0; 1_000]);
    assert!(meter.finish().is_none());
    assert!(
        BandMeter::new(2, RATE).finish().is_none(),
        "and neither does nothing"
    );
}
