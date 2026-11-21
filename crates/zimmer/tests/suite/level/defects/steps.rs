//! What a staircase must be to be reported, each limit met by one built just
//! inside it and missed by one just outside: how large a step, how many, how
//! regular — and how exactly what is found is measured.

use super::{RATE, found, tone};

/// Two seconds of a 440 Hz tone at 0.5, gained by `steps(n)` dB from half a
/// second on, where `n` counts the periods of `samples` since then.
fn staircase(samples: f64, steps: impl Fn(f64) -> f64) -> Vec<f32> {
    let period = samples / f64::from(RATE);
    tone(440.0, 0.5, 2.0, |t| {
        steps(((t - 0.5).max(0.0) / period).floor())
    })
}

/// Fifty-four steps of `size` dB each.
fn even(size: f64) -> impl Fn(f64) -> f64 {
    move |n| -size * n.min(54.0)
}

/// A quarter of a decibel a step is the smallest reported: three tenths are,
/// two tenths are not, however regular.
#[test]
fn a_step_is_at_least_a_quarter_of_a_decibel() {
    let reported = found(1, &staircase(800.0, even(0.3)));
    assert_eq!(reported.zipper.count, 1, "{reported:?}");
    let under = found(1, &staircase(800.0, even(0.2)));
    assert_eq!(under.zipper.count, 0, "{under:?}");
}

/// A staircase takes ten steps to be reported: nine are a sound's flutter
/// lining up as often as a game's fade.
#[test]
fn a_staircase_takes_ten_steps() {
    let ten = found(1, &staircase(800.0, |n| -2.0 * n.min(10.0)));
    assert_eq!(ten.zipper.count, 1, "{ten:?}");
    let nine = found(1, &staircase(800.0, |n| -2.0 * n.min(9.0)));
    assert_eq!(nine.zipper.count, 0, "{nine:?}");
}

/// Four in five steps must go the staircase's way by half its mean step: one
/// in every three under a third the size is too irregular.
#[test]
fn most_steps_go_the_staircases_way() {
    let ragged = |every: f64| move |n: f64| -(n - (n / every).floor() * 0.7).min(54.0);
    let reported = found(1, &staircase(800.0, ragged(5.0)));
    assert_eq!(reported.zipper.count, 1, "{reported:?}");
    let under = found(1, &staircase(800.0, ragged(3.0)));
    assert_eq!(under.zipper.count, 0, "{under:?}");
}

/// A staircase's period is refined to a fraction of a block of half a
/// millisecond, and its step is read to within 6%.
#[test]
fn a_staircase_is_measured_to_a_fraction_of_a_block() {
    for samples in [600.0, 800.0, 1_234.0] {
        for size in [0.5, 1.0] {
            let defects = found(1, &staircase(samples, even(size)));
            assert_eq!(defects.zipper.count, 1, "{samples} by {size}: {defects:?}");
            let run = &defects.zipper.worst[0];
            let period = run.period_seconds.expect("a period") * f64::from(RATE);
            assert!(
                (period - samples).abs() < 4.0,
                "{samples} by {size}: {run:?}"
            );
            assert!(
                (run.size_db / size - 1.0).abs() < 0.06,
                "{samples} by {size}: {run:?}"
            );
        }
    }
}

/// A staircase is placed at its first step, a period after half a second,
/// to within half a period.
#[test]
fn a_staircase_is_placed_at_its_first_step() {
    for samples in [600.0, 800.0] {
        let defects = found(1, &staircase(samples, even(1.0)));
        let run = &defects.zipper.worst[0];
        let period = samples / f64::from(RATE);
        let first = 0.5 + period;
        assert!((run.at_seconds - first).abs() < period / 2.0, "{run:?}");
        assert!((run.from_seconds - first).abs() < period / 2.0, "{run:?}");
    }
}

/// Steps that alternate in size, 1.5 dB then 0.6, repeat most strongly at
/// twice their period; the staircase is still found at its own.
#[test]
fn alternating_steps_are_found_at_their_own_period() {
    let alternating = |n: f64| -(1.5 * (n / 2.0).ceil() + 0.6 * (n / 2.0).floor()).min(60.0);
    let defects = found(1, &staircase(800.0, alternating));
    assert_eq!(defects.zipper.count, 1, "{defects:?}");
    let run = &defects.zipper.worst[0];
    let period = run.period_seconds.expect("a period") * f64::from(RATE);
    assert!((period - 800.0).abs() < 4.0, "{run:?}");
}

/// Wherever a period falls between two blocks, it is read to an eighth of a
/// block: three samples. Every period across one block, a sample apart.
#[test]
fn a_period_between_blocks_is_read_to_an_eighth_of_one() {
    for samples in 792..=816 {
        let samples = f64::from(samples);
        let defects = found(1, &staircase(samples, even(1.0)));
        assert_eq!(defects.zipper.count, 1, "{samples}: {defects:?}");
        let run = &defects.zipper.worst[0];
        let period = run.period_seconds.expect("a period") * f64::from(RATE);
        assert!((period - samples).abs() < 3.0, "{samples}: {run:?}");
    }
}
