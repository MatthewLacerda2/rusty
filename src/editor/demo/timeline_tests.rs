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
