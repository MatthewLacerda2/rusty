//! Where the energy sits, on signals whose spectrum is one frequency.
//!
//! A tone is the honest test for a band split: it has exactly one place its
//! energy can be, so an answer that puts it elsewhere is wrong rather than
//! imprecise.

use std::f32::consts::TAU;

use scorsese_zimmer::level::BandMeter;

/// The analysis rate the assertions are written against.
const RATE: u32 = 48_000;

/// A one-second sine at `hz`.
fn tone(hz: f32) -> Vec<f32> {
    (0..RATE)
        .map(|i| (TAU * hz * i as f32 / RATE as f32).sin() * 0.8)
        .collect()
}

/// Where a tone's energy lands, as shares.
fn shares(samples: &[f32]) -> (f64, f64, f64) {
    let mut meter = BandMeter::new(1, RATE);
    meter.feed(samples);
    let bands = meter.finish().expect("audible");
    (bands.low, bands.mid, bands.high)
}

/// The claim the "muddy" finding rests on: a bass tone reads as bass.
#[test]
fn a_low_tone_reports_its_energy_in_the_low_band() {
    let (low, mid, high) = shares(&tone(60.0));
    assert!(low > 0.9, "60 Hz is bass: low {low:.3}, mid {mid:.3}");
    assert!(high < 0.01, "and nothing up top: high {high:.3}");
}

/// And a midrange tone reads as midrange, so a mix that is *not* bass-heavy
/// does not read as one — the assertion that keeps the first test from passing
/// for a meter that answers "low" to everything.
#[test]
fn a_midrange_tone_reports_its_energy_in_the_mid_band() {
    let (low, mid, high) = shares(&tone(1_000.0));
    assert!(mid > 0.95, "1 kHz is midrange: mid {mid:.3}, low {low:.3}");
    assert!(high < 0.02, "high {high:.3}");
}

/// The top of the split, which is what "thin" and "harsh" are read off.
///
/// The threshold is looser than the bass one, and that is the crossover's
/// declared coarseness showing rather than a defect: a 6 dB/octave split leaves
/// a real share of a 12 kHz tone in the band below it. The finding this
/// supports is "most of this is up top", and that survives the bleed.
#[test]
fn a_high_tone_reports_its_energy_in_the_high_band() {
    let (low, mid, high) = shares(&tone(12_000.0));
    assert!(
        high > 0.75,
        "12 kHz is up top: high {high:.3}, mid {mid:.3}"
    );
    assert!(high > mid + low, "and it is where most of the energy is");
}

/// Three shares of one whole, so a report can say "42% of the energy is down
/// low" and mean it.
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

/// A silence has no spectral balance, and inventing one would put a row of
/// plausible percentages under a clip that makes no sound.
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
