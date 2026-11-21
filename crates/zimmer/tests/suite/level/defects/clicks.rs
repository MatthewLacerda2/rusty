//! Where a click starts: each threshold met exactly by a jump built sample by sample, and missed
//! just under it.

use super::{found, RATE};

fn step_out_of_silence(level: f32) -> Vec<f32> {
    let mut signal = vec![0.0; RATE as usize / 2];
    signal[RATE as usize / 4..].fill(level);
    signal
}

#[test]
fn the_smallest_click_is_a_hundredth_of_full_scale() {
    for (level, size_db) in [(0.01, 40.0), (0.1, 60.0)] {
        let defects = found(1, &step_out_of_silence(level));
        assert_eq!(defects.clicks.count, 1, "{level}: {defects:?}");
        let click = &defects.clicks.worst[0];
        assert!((click.size_db - size_db).abs() < 0.01, "{level}: {click:?}");
        assert_eq!(click.at_seconds, 0.25, "{level}: {click:?}");
    }
    let under = found(1, &step_out_of_silence(0.0099));
    assert_eq!(under.clicks.count, 0, "{under:?}");
}

fn zigzag_stepped_by(offset: f32) -> Vec<f32> {
    let half = RATE as usize / 4;
    (0..2 * half)
        .map(|i| {
            let zig = if i % 2 == 0 { 1.0 / 32.0 } else { -1.0 / 32.0 };
            zig + if i >= half { offset } else { 0.0 }
        })
        .collect()
}

#[test]
fn a_click_is_four_times_every_jump_around_it() {
    let defects = found(1, &zigzag_stepped_by(3.0 / 16.0));
    assert_eq!(defects.clicks.count, 1, "{defects:?}");
    let click = &defects.clicks.worst[0];
    assert!(
        (click.size_db - 20.0 * 4f64.log10()).abs() < 0.01,
        "{click:?}"
    );
    assert_eq!(click.at_seconds, 0.25, "{click:?}");

    let under = found(1, &zigzag_stepped_by(0.18));
    assert_eq!(under.clicks.count, 0, "3.9 times: {under:?}");
}

#[test]
fn a_click_at_the_very_end_is_found() {
    let mut signal = vec![0.0; RATE as usize / 2];
    let at = signal.len() - 100;
    signal[at..].fill(0.3);
    let defects = found(1, &signal);
    assert_eq!(defects.clicks.count, 1, "{defects:?}");
    assert_eq!(
        defects.clicks.worst[0].at_seconds,
        at as f64 / f64::from(RATE)
    );
}
