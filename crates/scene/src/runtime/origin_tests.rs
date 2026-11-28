use glam::{IVec3, Vec3};

use super::{FloatingOrigin, OriginSettings, ORIGIN_STEP};

fn on() -> FloatingOrigin {
    FloatingOrigin {
        settings: OriginSettings {
            enabled: true,
            threshold: 4096.0,
        },
        ..Default::default()
    }
}

#[test]
fn off_it_never_plans_a_shift() {
    let o = FloatingOrigin::default();
    assert_eq!(o.plan(Vec3::splat(1.0e6)), None);
}

#[test]
fn in_range_or_lost_plans_nothing() {
    let o = on();
    assert_eq!(o.plan(Vec3::new(4096.0, -4000.0, 0.0)), None);
    assert_eq!(o.plan(Vec3::new(f32::NAN, 0.0, 0.0)), None);
    assert_eq!(o.plan(Vec3::new(f32::INFINITY, 0.0, 0.0)), None);
}

#[test]
fn only_the_far_axes_shift_by_whole_steps() {
    let o = on();
    let steps = o.plan(Vec3::new(30_000.0, 3000.0, -5000.0)).unwrap();
    assert_eq!(steps, IVec3::new(29, 0, -5));
}

#[test]
fn a_shift_brings_the_focus_within_half_a_step_and_adds_up() {
    let mut o = on();
    let at = Vec3::new(30_000.0, 0.0, 0.0);
    let by = o.shift(o.plan(at).unwrap());
    assert!((at + by).x.abs() <= ORIGIN_STEP / 2.0, "{}", at + by);
    assert_eq!(o.offset().x, 29.0 * 1024.0);
    o.shift(IVec3::new(1, 0, 0));
    assert_eq!(o.offset_f32(), Vec3::new(30.0 * 1024.0, 0.0, 0.0));
    assert_eq!(o.last_shift(), Vec3::new(-1024.0, 0.0, 0.0));
    assert_eq!(o.serial(), 2);
}

#[test]
fn a_far_float_less_a_whole_step_is_exact() {
    // The lattice is a power of two, so the subtraction rounds nothing.
    for x in [32_768.004_f32, 30_000.123, 65_000.7, -40_960.03] {
        let steps = (x / ORIGIN_STEP).round();
        let back = (x - steps * ORIGIN_STEP) + steps * ORIGIN_STEP;
        assert_eq!(back.to_bits(), x.to_bits(), "{x}");
    }
}

#[test]
fn reset_returns_to_authored_space_and_keeps_the_count() {
    let mut o = on();
    o.focus = Some(3);
    o.shift(IVec3::new(4, 0, 0));
    o.reset();
    assert!(o.is_zero());
    assert_eq!(o.focus, None);
    assert_eq!(o.serial(), 1, "a reader still sees the shift it missed");
}
