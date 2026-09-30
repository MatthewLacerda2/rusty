//! `Tween::sample` (#424): endpoints land exactly, loops and yoyo end where they
//! should, and an endless tween never finishes.

use glam::Vec4;

use super::{Ease, Property, Tween};

fn tween(duration: f64, loops: Option<u32>, yoyo: bool) -> Tween {
    Tween {
        property: Property::Alpha,
        from: None,
        to: Vec4::splat(10.0),
        duration,
        ease: Ease::Linear,
        loops,
        yoyo,
        on_complete: None,
        group: None,
    }
}

fn x(t: &Tween, at: f64) -> (f32, bool) {
    let (v, done) = t.sample(Vec4::ZERO, at);
    (v.x, done)
}

#[test]
fn a_single_tween_interpolates_then_lands_exactly_on_its_target() {
    let t = tween(1.0, Some(1), false);
    assert_eq!(x(&t, 0.0), (0.0, false));
    assert_eq!(x(&t, 0.25), (2.5, false));
    assert_eq!(x(&t, 1.0), (10.0, true));
    assert_eq!(
        x(&t, 3.0),
        (10.0, true),
        "overshooting time clamps to the end"
    );
}

#[test]
fn loops_restart_and_yoyo_reverses() {
    let restart = tween(1.0, Some(3), false);
    assert_eq!(x(&restart, 1.25), (2.5, false));
    assert_eq!(x(&restart, 3.0), (10.0, true));
    let yoyo = tween(1.0, Some(2), true);
    assert_eq!(x(&yoyo, 0.5), (5.0, false));
    assert_eq!(
        x(&yoyo, 1.25),
        (7.5, false),
        "the second cycle plays backwards"
    );
    assert_eq!(x(&yoyo, 2.0), (0.0, true), "an even yoyo ends back home");
    let odd = tween(1.0, Some(3), true);
    assert_eq!(x(&odd, 3.0), (10.0, true), "an odd yoyo ends at the target");
}

#[test]
fn an_endless_tween_never_finishes() {
    let t = tween(0.5, None, true);
    assert_eq!(x(&t, 1000.25), (5.0, false));
}

#[test]
fn a_zero_duration_tween_jumps_to_its_target() {
    assert_eq!(x(&tween(0.0, Some(1), false), 0.0), (10.0, true));
}

#[test]
fn eased_values_overshoot_past_the_target() {
    let mut t = tween(1.0, Some(1), false);
    t.ease = Ease::parse("back_out").unwrap();
    assert!(x(&t, 0.6).0 > 10.0);
}

#[test]
fn property_paths_resolve_and_report_their_arity() {
    assert_eq!(Property::parse("Image.color").map(Property::arity), Some(4));
    assert_eq!(
        Property::parse("Transform.position").map(Property::arity),
        Some(3)
    );
    assert_eq!(
        Property::parse("CanvasGroup.alpha").map(Property::path),
        Some("CanvasGroup.alpha")
    );
    assert_eq!(Property::parse("CanvasGroup.Alpha"), None);
    assert!(Property::paths().contains("RectTransform.anchored_position"));
}
