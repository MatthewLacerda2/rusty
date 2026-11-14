use super::*;

#[test]
fn the_bar_maps_ticks_to_x_and_back() {
    for tick in [0, 1, 37, 99] {
        let x = x_of(tick, 10.0, 210.0, 100);
        assert_eq!(tick_at(x, 10.0, 210.0, 100), tick);
    }
}

#[test]
fn a_pointer_off_the_bar_clamps_to_its_ends() {
    assert_eq!(tick_at(-50.0, 10.0, 210.0, 100), 0);
    assert_eq!(tick_at(900.0, 10.0, 210.0, 100), 99);
}

#[test]
fn degenerate_demos_and_bars_do_not_divide_by_zero() {
    assert_eq!(tick_at(5.0, 0.0, 10.0, 0), 0);
    assert_eq!(tick_at(5.0, 10.0, 10.0, 50), 0);
    assert_eq!(tick_at(f32::NAN, 0.0, 10.0, 50), 0);
    // A one-tick demo has one place to stand.
    assert!(x_of(0, 0.0, 100.0, 1).is_finite());
    assert!(x_of(9, 0.0, 100.0, 0).is_finite());
}

#[test]
fn a_mark_is_found_near_its_tick_only() {
    let marks = vec![(50, "took_damage".to_string())];
    let x = x_of(50, 0.0, 100.0, 101);
    assert_eq!(
        mark_near(&marks, x + 2.0, 0.0, 100.0, 101),
        Some("took_damage")
    );
    assert_eq!(mark_near(&marks, x + 30.0, 0.0, 100.0, 101), None);
}

#[test]
fn playback_spends_wall_time_on_the_recorded_dt() {
    // A 144 Hz recording on a 60 Hz display: 144 ticks in 60 frames, not 60.
    let mut pace = Pace::new(vec![1.0 / 144.0; 400]);
    let ran: u64 = (0..60).map(|_| pace.spend(0, 1.0 / 60.0)).sum::<u64>();
    assert!((143..=144).contains(&ran), "{ran}");
}

#[test]
fn a_stall_a_broken_dt_and_the_end_never_run_away() {
    let mut pace = Pace::new(vec![0.01; 1000]);
    let stall = pace.spend(0, 5.0);
    assert!(
        (9..=10).contains(&stall),
        "a stall spends at most 0.1 s: {stall}"
    );
    let mut nan = Pace::new(vec![f32::NAN; 1000]);
    assert_eq!(nan.spend(0, 0.016), 64, "a NaN costs nothing, capped");
    let mut end = Pace::new(vec![0.01; 3]);
    assert_eq!(end.spend(2, 1.0), 0, "the last tick holds still");
    assert_eq!(end.spend(0, 0.0), 0, "and keeps no budget");
}
