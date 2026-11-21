//! Integrated loudness, held to EBU Tech 3341's reference signals.

use std::f32::consts::TAU;

use scorsese_zimmer::level::Integrated;

const TOLERANCE: f64 = 0.1;

const RATE: u32 = 48_000;

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

#[test]
fn quiet_passages_either_side_are_gated_out() {
    let mut signal = tone(-36.0, 10.0);
    signal.extend(tone(-23.0, 60.0));
    signal.extend(tone(-36.0, 10.0));
    let read = measured(&signal, RATE).expect("audible");
    assert!((read + 23.0).abs() <= TOLERANCE, "read {read} LUFS");
}

#[test]
fn silence_is_left_out_of_the_answer() {
    let mut signal = vec![0.0; 2 * RATE as usize * 3];
    signal.extend(tone(-23.0, 20.0));
    let read = measured(&signal, RATE).expect("audible");
    assert!((read + 23.0).abs() <= TOLERANCE, "read {read} LUFS");
}

#[test]
fn silence_and_a_blip_have_no_loudness() {
    assert_eq!(measured(&vec![0.0; 2 * RATE as usize], RATE), None);
    assert_eq!(measured(&tone(-10.0, 0.3), RATE), None);
}

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

#[test]
fn runs_add_up_to_the_whole() {
    let signal = tone(-20.0, 2.0);
    let mut meter = Integrated::new(2, RATE);
    for run in signal.chunks(2 * 777) {
        meter.feed(run);
    }
    assert_eq!(meter.finish(), measured(&signal, RATE));
}
