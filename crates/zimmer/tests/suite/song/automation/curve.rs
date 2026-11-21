//! The arithmetic of a curve, by the number.
//!
//! Nothing here renders anything. `value_at` is the whole mechanism — every
//! parameter that moves is this one function read at a different moment — so
//! it is asserted against the values it should produce rather than against a
//! mix that happens to change.

use scorsese_zimmer::song::{Easing, Param};

use super::setup::{at, curve, eased};

/// Every easing at the midpoint of one segment, and the whole point of the
/// list: they must not all be the same number.
#[test]
fn each_easing_bends_the_same_segment_its_own_way() {
    for (easing, expected) in [
        (Easing::Linear, 0.5),
        (Easing::EaseIn, 0.25),
        (Easing::EaseOut, 0.75),
        (Easing::EaseInOut, 0.5),
        (Easing::Hold, 0.0),
    ] {
        let moving = curve(Param::Gain, vec![eased(0.0, 0.0, easing), at(4.0, 1.0)]);
        let midpoint = moving.value_at(2.0).expect("the curve has points");
        assert!(
            (midpoint - expected).abs() < 1e-6,
            "{easing:?} reads {midpoint} halfway, not {expected}"
        );
    }
}

/// `ease_in_out` and `linear` agree at the midpoint by construction, so the
/// quarter is where the two are told apart — otherwise the test above would
/// pass on a smoothstep that had quietly become a straight line.
#[test]
fn easing_in_and_out_is_slow_at_both_ends_and_quick_between() {
    let smooth = curve(
        Param::Gain,
        vec![eased(0.0, 0.0, Easing::EaseInOut), at(4.0, 1.0)],
    );
    let quarter = smooth.value_at(1.0).expect("the curve has points");
    let three_quarters = smooth.value_at(3.0).expect("the curve has points");
    assert!(
        (quarter - 0.15625).abs() < 1e-6,
        "a quarter in reads {quarter}"
    );
    assert!(
        (three_quarters - 0.84375).abs() < 1e-6,
        "three quarters in reads {three_quarters}"
    );
}

/// A held segment does not travel at all — it jumps at the point that ends it.
#[test]
fn a_held_segment_jumps_at_the_next_point_rather_than_travelling() {
    let stepped = curve(
        Param::Gain,
        vec![eased(0.0, 0.2, Easing::Hold), at(4.0, 0.9)],
    );
    assert_eq!(stepped.value_at(3.999), Some(0.2), "it has not moved yet");
    assert_eq!(stepped.value_at(4.0), Some(0.9), "and now it has");
}

/// The boundary, which is where a segment search goes wrong quietly: a beat
/// landing *exactly* on an interior point belongs to the segment that starts
/// there, not to the one that ends there.
///
/// `hold` is what asks it, because every other easing arrives at the same
/// number from both sides — the two readings of a boundary are only telling
/// apart when the segment before it did not travel.
#[test]
fn a_beat_exactly_on_a_point_reads_the_segment_that_starts_there() {
    let stepped = curve(
        Param::Gain,
        vec![
            eased(0.0, 0.125, Easing::Hold),
            at(4.0, 0.75),
            at(8.0, 0.25),
        ],
    );
    assert_eq!(
        stepped.value_at(3.999),
        Some(0.125),
        "the step has not come"
    );
    assert_eq!(stepped.value_at(4.0), Some(0.75), "and now it has");
    assert_eq!(stepped.value_at(6.0), Some(0.5), "then it travels on");
}

/// Outside the written span the value holds. Extrapolating would make a
/// two-point build keep climbing forever, which is nobody's reading of two
/// points.
#[test]
fn a_beat_outside_the_written_span_clamps_rather_than_extrapolating() {
    let build = curve(Param::Gain, vec![at(4.0, 0.25), at(8.0, 0.75)]);
    assert_eq!(build.value_at(0.0), Some(0.25), "before the first point");
    assert_eq!(build.value_at(-99.0), Some(0.25), "far before it");
    assert_eq!(build.value_at(8.0), Some(0.75), "on the last point");
    assert_eq!(build.value_at(4000.0), Some(0.75), "and long past it");
}

/// One point is a constant, which is a legitimate thing to write: an offset
/// that says what a parameter is for the whole piece.
#[test]
fn one_point_holds_for_the_whole_piece() {
    let parked = curve(Param::Cutoff, vec![at(6.0, 900.0)]);
    for beat in [0.0, 6.0, 12.0] {
        assert_eq!(parked.value_at(beat), Some(900.0), "at beat {beat}");
    }
}

/// The middle segment of three, so the search picks the pair a beat is in
/// rather than the first pair or the last.
#[test]
fn a_beat_is_read_against_the_segment_it_falls_in() {
    let shape = curve(
        Param::Gain,
        vec![at(0.0, 0.0), at(2.0, 1.0), at(6.0, 1.0), at(8.0, 0.0)],
    );
    assert_eq!(shape.value_at(1.0), Some(0.5), "rising");
    assert_eq!(shape.value_at(4.0), Some(1.0), "held between two equals");
    assert_eq!(shape.value_at(7.0), Some(0.5), "falling");
}

/// A curve with nothing in it reads as nothing, so whatever asks leaves the
/// number the document already wrote alone. Validation refuses one, but the
/// evaluator must not invent a zero on the way there.
#[test]
fn a_curve_with_no_points_says_nothing() {
    assert_eq!(curve(Param::Gain, vec![]).value_at(0.0), None);
}
