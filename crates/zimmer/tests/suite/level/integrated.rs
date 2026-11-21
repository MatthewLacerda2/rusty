//! Integrated loudness, held to EBU Tech 3341's reference signals.
//!
//! The standard's own test cases: a stereo 1 kHz tone at a stated peak level
//! reads that level in LUFS, and a quiet passage either side of a loud one is
//! gated out of the answer. At the case's own durations: a block that
//! straddles a quiet passage and a loud one passes the gate on the loud part,
//! and the answer is only within tolerance when there are enough whole blocks
//! to outweigh the few that straddle.

use std::f32::consts::TAU;

use scorsese_zimmer::level::Integrated;

/// Tech 3341 allows ±0.1 LU.
const TOLERANCE: f64 = 0.1;

const RATE: u32 = 48_000;

/// A stereo 1 kHz sine whose peak sits at `dbfs`, for `seconds`.
fn tone(dbfs: f32, seconds: f32) -> Vec<f32> {
    let amplitude = 10_f32.powf(dbfs / 20.0);
    let frames = (seconds * RATE as f32) as usize;
    (0..frames)
        .flat_map(|i| {
            let s = amplitude * (TAU * 1_000.0 * i as f32 / RATE as f32).sin();
            [s, s]
        })
        .collect()
}

fn measured(samples: &[f32], rate: u32) -> Option<f64> {
    let mut meter = Integrated::new(2, rate);
    meter.feed(samples);
    meter.finish()
}

/// Tech 3341 case 1 and 2: −23 dBFS reads −23 LUFS, −33 reads −33.
#[test]
fn a_reference_tone_reads_its_own_level() {
    for level in [-23.0, -33.0] {
        let read = measured(&tone(level, 5.0), RATE).expect("audible");
        assert!(
            (read - f64::from(level)).abs() <= TOLERANCE,
            "{level} dBFS read {read} LUFS"
        );
    }
}

/// Tech 3341 case 3: −36, −23, −36 for 10 s, 60 s, 10 s. The quiet ends sit
/// under the relative gate, so the answer is the loud middle's.
#[test]
fn quiet_passages_either_side_are_gated_out() {
    let mut signal = tone(-36.0, 10.0);
    signal.extend(tone(-23.0, 60.0));
    signal.extend(tone(-36.0, 10.0));
    let read = measured(&signal, RATE).expect("audible");
    assert!((read + 23.0).abs() <= TOLERANCE, "read {read} LUFS");
}

/// Silence ahead of the programme does not make it read quieter: the
/// absolute gate is what makes an ad's first second of nothing not count.
#[test]
fn silence_is_left_out_of_the_answer() {
    let mut signal = vec![0.0; 2 * RATE as usize * 3];
    signal.extend(tone(-23.0, 20.0));
    let read = measured(&signal, RATE).expect("audible");
    assert!((read + 23.0).abs() <= TOLERANCE, "read {read} LUFS");
}

/// Nothing to measure is `None`, not a very negative number: all silence, and
/// a signal too short to fill one 400 ms block.
#[test]
fn silence_and_a_blip_have_no_loudness() {
    assert_eq!(measured(&vec![0.0; 2 * RATE as usize], RATE), None);
    assert_eq!(measured(&tone(-10.0, 0.3), RATE), None);
}

/// The filters are designed for the rate they run at: the same tone read at
/// 44.1 kHz gives the same answer.
#[test]
fn the_weighting_follows_the_rate() {
    let rate = 44_100;
    let samples: Vec<f32> = (0..rate as usize * 5)
        .flat_map(|i| {
            let s = 10_f32.powf(-23.0 / 20.0) * (TAU * 1_000.0 * i as f32 / rate as f32).sin();
            [s, s]
        })
        .collect();
    let read = measured(&samples, rate).expect("audible");
    assert!((read + 23.0).abs() <= TOLERANCE, "read {read} LUFS");
}

/// Fed in uneven runs, it reads exactly what it reads fed whole.
#[test]
fn runs_add_up_to_the_whole() {
    let signal = tone(-20.0, 2.0);
    let mut meter = Integrated::new(2, RATE);
    for run in signal.chunks(2 * 777) {
        meter.feed(run);
    }
    assert_eq!(meter.finish(), measured(&signal, RATE));
}
