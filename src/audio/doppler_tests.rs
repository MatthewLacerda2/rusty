//! The doppler rate (#1749): the physical shift, its level, its clamp, and the
//! teleport rule.

use glam::Vec3;

use super::*;

const C: f32 = SPEED_OF_SOUND;

fn still(at: Vec3) -> Body {
    Body {
        position: at,
        velocity: Vec3::ZERO,
    }
}

/// A source 100 m down -X moving along +X at `speed` (toward a listener at the origin).
fn source(speed: f32) -> Body {
    Body {
        position: Vec3::new(-100.0, 0.0, 0.0),
        velocity: Vec3::new(speed, 0.0, 0.0),
    }
}

#[test]
fn a_source_closing_at_50_ms_plays_343_over_293() {
    let r = rate(still(Vec3::ZERO), source(50.0), C, 1.0);
    assert!((r - 343.0 / 293.0).abs() < 1e-5, "{r}");
}

#[test]
fn a_source_leaving_at_50_ms_plays_343_over_393() {
    let r = rate(still(Vec3::ZERO), source(-50.0), C, 1.0);
    assert!((r - 343.0 / 393.0).abs() < 1e-5, "{r}");
}

#[test]
fn a_listener_closing_on_a_still_source_raises_the_pitch() {
    let listener = Body {
        position: Vec3::ZERO,
        velocity: Vec3::new(-50.0, 0.0, 0.0),
    };
    let r = rate(listener, still(Vec3::new(-100.0, 0.0, 0.0)), C, 1.0);
    assert!((r - 393.0 / 343.0).abs() < 1e-5, "{r}");
}

#[test]
fn level_zero_and_a_still_pair_play_at_one() {
    assert_eq!(rate(still(Vec3::ZERO), source(50.0), C, 0.0), 1.0);
    let at = Vec3::new(3.0, 0.0, 0.0);
    assert_eq!(rate(still(Vec3::ZERO), still(at), C, 1.0), 1.0);
}

#[test]
fn motion_across_the_line_of_sight_does_not_shift() {
    let s = Body {
        position: Vec3::new(-100.0, 0.0, 0.0),
        velocity: Vec3::new(0.0, 0.0, 50.0),
    };
    assert_eq!(rate(still(Vec3::ZERO), s, C, 1.0), 1.0);
}

#[test]
fn a_higher_level_exaggerates_and_the_rate_is_clamped() {
    let doubled = rate(still(Vec3::ZERO), source(50.0), C, 2.0);
    assert!((doubled - 343.0 / 243.0).abs() < 1e-5, "{doubled}");
    assert_eq!(rate(still(Vec3::ZERO), source(340.0), C, 1.0), RATE_MAX);
    assert_eq!(rate(still(Vec3::ZERO), source(-1000.0), C, 1.0), RATE_MIN);
}

#[test]
fn a_co_located_pair_or_a_nan_plays_at_one() {
    assert_eq!(rate(still(Vec3::ZERO), source(50.0), C, f32::NAN), 1.0);
    let nan = Body {
        position: Vec3::ZERO,
        velocity: Vec3::NAN,
    };
    assert_eq!(rate(still(Vec3::ZERO), still(Vec3::ZERO), C, 1.0), 1.0);
    assert_eq!(rate(still(Vec3::X), nan, C, 1.0), 1.0);
}

#[test]
fn a_move_past_the_teleport_speed_is_still() {
    let v = velocity(Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0), 0.01);
    assert!((v.x - 50.0).abs() < 1e-3);
    let supersonic = velocity(Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0), 0.01);
    assert!(
        (supersonic.x - 400.0).abs() < 1e-2,
        "faster than sound is motion"
    );
    let jump = Vec3::new(100.0, 0.0, 0.0);
    assert_eq!(velocity(Vec3::ZERO, jump, 0.01), Vec3::ZERO, "a 100 m jump");
    assert_eq!(velocity(Vec3::ZERO, Vec3::X, 0.0), Vec3::ZERO, "no time");
    assert_eq!(velocity(Vec3::ZERO, Vec3::NAN, 0.01), Vec3::ZERO);
}
