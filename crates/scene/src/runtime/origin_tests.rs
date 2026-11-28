use glam::{IVec3, Vec3};

use super::{FloatingOrigin, OriginSettings, ORIGIN_STEP};

const ON: OriginSettings = OriginSettings {
    enabled: true,
    threshold: 4096.0,
};

fn on() -> FloatingOrigin {
    FloatingOrigin::default()
}

#[test]
fn off_it_never_plans_a_shift() {
    let o = FloatingOrigin::default();
    assert_eq!(o.plan(Vec3::splat(1.0e6), &OriginSettings::default()), None);
}

#[test]
fn in_range_or_lost_plans_nothing() {
    let o = on();
    assert_eq!(o.plan(Vec3::new(4096.0, -4000.0, 0.0), &ON), None);
    assert_eq!(o.plan(Vec3::new(f32::NAN, 0.0, 0.0), &ON), None);
    assert_eq!(o.plan(Vec3::new(f32::INFINITY, 0.0, 0.0), &ON), None);
}

#[test]
fn only_the_far_horizontal_axes_shift_by_whole_steps() {
    let o = on();
    let steps = o.plan(Vec3::new(30_000.0, 9000.0, -5000.0), &ON).unwrap();
    assert_eq!(steps, IVec3::new(29, 0, -5));
    assert_eq!(
        o.plan(Vec3::new(0.0, 20_000.0, 0.0), &ON),
        None,
        "never up or down"
    );
}

#[test]
fn a_shift_brings_the_focus_within_half_a_step_and_adds_up() {
    let mut o = on();
    let at = Vec3::new(30_000.0, 0.0, 0.0);
    let by = o.shift(o.plan(at, &ON).unwrap());
    assert!((at + by).x.abs() <= ORIGIN_STEP / 2.0, "{}", at + by);
    assert_eq!(o.offset().x, 29.0 * 1024.0);
    assert_eq!(o.shift(IVec3::new(1, 0, 0)), Vec3::new(-1024.0, 0.0, 0.0));
    assert_eq!(o.offset_f32(), Vec3::new(30.0 * 1024.0, 0.0, 0.0));
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
