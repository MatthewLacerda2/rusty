//! Pad naming, dead zones, rumble records, and axes' per-tick sampling (#471).

use super::*;
use crate::core::input::InputState;

#[test]
fn pad_zero_is_unnumbered_and_later_pads_carry_their_index() {
    assert_eq!(pad_name(0, "A"), "PADA");
    assert_eq!(pad_name(0, "LEFTX"), "PADLEFTX");
    assert_eq!(pad_name(1, "A"), "PAD1A");
    assert_eq!(pad_name(3, "RIGHTTRIGGER"), "PAD3RIGHTTRIGGER");
}

#[test]
fn radial_dead_zone_cuts_the_centre_and_rescales_the_rest() {
    assert_eq!(radial_dead_zone(0.1, 0.1, 0.15), (0.0, 0.0));
    // Past the edge the output starts from zero, and full travel reads 1.
    let (x, y) = radial_dead_zone(0.0, 1.0, 0.15);
    assert_eq!((x, y), (0.0, 1.0));
    let (x, _) = radial_dead_zone(0.575, 0.0, 0.15);
    assert!(
        (x - 0.5).abs() < 1e-6,
        "0.575 is halfway from 0.15 to 1: {x}"
    );
    // The direction is kept, including its sign.
    let (x, y) = radial_dead_zone(-0.6, 0.6, 0.15);
    assert!(x < 0.0 && y > 0.0 && (x + y).abs() < 1e-6);
    // A diagonal past the unit circle clamps to magnitude 1.
    let (x, y) = radial_dead_zone(1.0, 1.0, 0.15);
    assert!((x.hypot(y) - 1.0).abs() < 1e-6);
}

#[test]
fn axial_dead_zone_rescales_one_value() {
    assert_eq!(axial_dead_zone(0.04, 0.05), 0.0);
    assert_eq!(axial_dead_zone(1.0, 0.05), 1.0);
    assert!((axial_dead_zone(0.525, 0.05) - 0.5).abs() < 1e-6);
    assert!((axial_dead_zone(-0.525, 0.05) + 0.5).abs() < 1e-6);
}

#[test]
fn dead_zones_clamp_so_full_travel_still_reads_one() {
    let zones = DeadZones::clamped(2.0, -1.0);
    assert_eq!(zones.stick, 0.95);
    assert_eq!(zones.trigger, 0.0);
    assert_eq!(radial_dead_zone(1.0, 0.0, zones.stick).0, 1.0);
}

#[test]
fn rumble_is_recorded_queued_and_drained() {
    let mut pads = PadRecords::default();
    pads.request_rumble(1, Rumble::new(2.0, 0.5, -1.0));
    let expected = Rumble {
        low: 1.0,
        high: 0.5,
        seconds: 0.0,
    };
    assert_eq!(pads.last_rumble(1), expected);
    assert_eq!(pads.last_rumble(0), Rumble::default());
    assert_eq!(pads.take_rumble(), vec![(1, expected)]);
    assert!(pads.take_rumble().is_empty(), "drained once");
    assert_eq!(
        pads.last_rumble(1),
        expected,
        "the record outlives the queue"
    );
    // An out-of-range pad is ignored rather than panicking.
    pads.request_rumble(MAX_PADS, Rumble::new(1.0, 1.0, 1.0));
    assert!(pads.take_rumble().is_empty());
}

#[test]
fn connection_is_per_slot() {
    let mut pads = PadRecords::default();
    pads.set_connected(2, true);
    assert!(pads.is_connected(2));
    assert!(!pads.is_connected(0));
    pads.set_connected(MAX_PADS, true);
    assert!(!pads.is_connected(MAX_PADS));
}

#[test]
fn an_axis_is_sampled_at_the_tick_boundary_and_holds() {
    let mut input = InputState::new();
    input.set_axis("PadLeftX", 0.5);
    assert_eq!(input.axis("PADLEFTX"), 0.0, "waits for the next tick");
    input.begin_tick();
    assert_eq!(input.axis("padleftx"), 0.5);
    input.begin_tick();
    assert_eq!(input.axis("PADLEFTX"), 0.5, "a level, not an edge");
    input.set_axis("PADLEFTX", -3.0);
    input.set_axis("PADLEFTY", f32::NAN);
    input.begin_tick();
    assert_eq!(input.axis("PADLEFTX"), -1.0, "clamped");
    assert_eq!(input.axis("PADLEFTY"), 0.0, "NaN reads as rest");
    assert_eq!(input.axis("NEVERWRITTEN"), 0.0);
}
