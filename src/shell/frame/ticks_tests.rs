//! The accumulator alone: whole ticks out, the remainder kept, the clamp.

use super::*;

#[test]
fn a_frame_shorter_than_a_tick_owes_nothing_until_the_time_adds_up() {
    let mut ticks = FixedTicks::default();
    assert_eq!(ticks.owe(0.007), 0);
    assert_eq!(ticks.owe(0.007), 0);
    assert_eq!(ticks.owe(0.007), 1, "21 ms owes one 16.7 ms tick");
    assert!((ticks.alpha() - (0.021 - FIXED_DELTA_TIME) / FIXED_DELTA_TIME).abs() < 1e-4);
}

#[test]
fn frames_on_the_tick_owe_one_each_for_an_hour() {
    let mut ticks = FixedTicks::default();
    let total: u64 = (0..216_000).map(|_| u64::from(ticks.owe(FIXED_DELTA_TIME))).sum();
    assert_eq!(total, 216_000);
}

#[test]
fn a_hitch_is_clamped_to_the_maximum_delta_time() {
    let mut ticks = FixedTicks::default();
    assert_eq!(ticks.owe(0.5), 20, "1/3 s at most: 20 ticks");
    assert_eq!(ticks.owe(3600.0), 20, "an hour asleep is still 20");
    assert!(ticks.alpha() < 1.0);
}

#[test]
fn nonsense_time_owes_nothing() {
    let mut ticks = FixedTicks::default();
    assert_eq!(ticks.owe(-1.0), 0);
    assert_eq!(ticks.owe(f32::NAN), 0);
    assert_eq!(ticks.alpha(), 0.0);
}

#[test]
fn clear_drops_the_remainder() {
    let mut ticks = FixedTicks::default();
    ticks.owe(0.01);
    ticks.clear();
    assert_eq!(ticks.alpha(), 0.0);
    assert_eq!(ticks.owe(0.01), 0);
}
