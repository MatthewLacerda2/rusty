use super::*;
use crate::dev::bench::Summary;

#[test]
fn speed_is_the_reference_over_the_probe() {
    assert_eq!(speed(Some(REFERENCE_MS), Some(REFERENCE_MS)), Some(1.0));
    // Twice as slow as the reference: half the speed.
    let slow = 2.0 * REFERENCE_MS;
    assert_eq!(speed(Some(slow), Some(slow)), Some(0.5));
}

#[test]
fn speed_averages_the_probes_on_either_side_of_the_frame() {
    // Probes of 1x and 3x the reference average to 2x: half the speed.
    let got = speed(Some(REFERENCE_MS), Some(3.0 * REFERENCE_MS));
    assert_eq!(got, Some(0.5));
}

#[test]
fn speed_uses_the_one_probe_that_read() {
    let slow = 4.0 * REFERENCE_MS;
    assert_eq!(speed(Some(slow), None), Some(0.25));
    assert_eq!(speed(None, Some(slow)), Some(0.25));
    assert_eq!(speed(None, None), None);
}

#[test]
fn cpu_speed_is_the_cpu_reference_over_the_probes_mean() {
    let r = CPU_REFERENCE_MS;
    assert_eq!(cpu_speed(r, r), Some(1.0));
    assert_eq!(cpu_speed(r, 3.0 * r), Some(0.5));
    // A probe that read nothing, or nonsense, is not a reading.
    assert_eq!(cpu_speed(0.0, 0.0), None);
    assert_eq!(cpu_speed(f64::NAN, r), None);
}

#[test]
fn the_cpu_probe_takes_well_under_a_millisecond() {
    cpu_probe_ms();
    let best = (0..5).map(|_| cpu_probe_ms()).fold(f64::MAX, f64::min);
    // A debug build runs the chain far slower than the bench's release one.
    assert!(best > 0.0 && best < 20.0, "{best} ms");
}

#[test]
fn spread_is_the_p5_to_p95_range_over_the_median() {
    let flat = [100.0; 20];
    assert_eq!(spread(&flat), 0.0);
    let ramp: Vec<f64> = (0..=100).map(f64::from).collect();
    assert_eq!(spread(&ramp), 1.8);
    assert_eq!(spread(&[5.0]), 0.0);
    assert_eq!(spread(&[0.0, 0.0, 0.0]), 0.0);
}

fn samples(clock: &[f64], cpu: &[f64]) -> Samples {
    let mut s = Samples::default();
    clock.iter().for_each(|&v| s.push("cpu_clock_pct", v));
    cpu.iter().for_each(|&v| s.push("cpu_ms", v));
    s
}

#[test]
fn a_moving_clock_or_a_noisy_cpu_time_earns_a_warning() {
    assert!(cpu_warnings(&samples(&[80.0; 10], &[1.5; 10])).is_empty());
    let moving: Vec<f64> = (0..20).map(|i| 40.0 + 3.0 * f64::from(i)).collect();
    let w = cpu_warnings(&samples(&moving, &[1.5; 20]));
    assert!(w.len() == 1 && w[0].contains("CPU clock moved"), "{w:?}");
    let noisy: Vec<f64> = (0..20).map(|i| 1.0 + 0.2 * f64::from(i)).collect();
    let w = cpu_warnings(&samples(&[80.0; 20], &noisy));
    assert!(w.len() == 1 && w[0].contains("cpu_ms spread"), "{w:?}");
}

#[test]
fn a_clock_that_moved_since_the_last_run_earns_a_warning() {
    let at = |pct: f64| Report {
        metrics: vec![("cpu_clock_pct".to_string(), Summary::of(&[pct]))],
        ..Report::default()
    };
    assert_eq!(cpu_drift(&at(80.0), None), None);
    assert_eq!(cpu_drift(&at(80.0), Some(&at(85.0))), None);
    let w = cpu_drift(&at(40.0), Some(&at(80.0))).unwrap();
    assert!(w.contains("-50%"), "{w}");
    // A last run without the row (an older report) compares nothing.
    assert_eq!(cpu_drift(&at(40.0), Some(&Report::default())), None);
}
