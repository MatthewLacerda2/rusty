//! The decal record's lifecycle (#639): opacity over a lifetime and fade, expiry,
//! and retiring early without popping back to whole.

use glam::Vec3;

use super::{Decal, DecalSpec};

fn stamped(lifetime: Option<f32>, fade: f32) -> Decal {
    let spec = DecalSpec {
        lifetime,
        fade,
        ..DecalSpec::default()
    };
    Decal::from_hit(Vec3::ZERO, Vec3::Y, spec)
}

#[test]
fn a_decal_without_a_lifetime_stays_whole() {
    let mut d = stamped(None, 2.0);
    d.age = 1.0e6;
    assert_eq!(d.opacity(), 1.0);
    assert!(!d.expired());
}

#[test]
fn opacity_fades_over_the_last_fade_seconds_of_the_lifetime() {
    let mut d = stamped(Some(4.0), 2.0);
    for (age, want) in [(0.0, 1.0), (2.0, 1.0), (3.0, 0.5), (4.0, 0.0)] {
        d.age = age;
        assert!(
            (d.opacity() - want).abs() < 1e-6,
            "age {age}: {}",
            d.opacity()
        );
    }
    assert!(d.expired());
}

#[test]
fn without_a_fade_it_is_whole_until_its_lifetime_ends() {
    let mut d = stamped(Some(1.0), 0.0);
    d.age = 0.99;
    assert_eq!(d.opacity(), 1.0);
    d.age = 1.0;
    assert_eq!(d.opacity(), 0.0);
    assert!(d.expired());
}

#[test]
fn retiring_fades_from_the_current_opacity_never_popping_back() {
    let mut d = stamped(Some(10.0), 4.0);
    d.age = 8.0; // half faded on its own
    d.retire(1.0);
    assert!(d.retiring);
    assert!((d.opacity() - 0.5).abs() < 1e-6, "{}", d.opacity());
    d.age = 8.5;
    assert!((d.opacity() - 0.25).abs() < 1e-6, "{}", d.opacity());
    d.age = 9.0;
    assert!(d.expired());
}

#[test]
fn retiring_keeps_an_earlier_end() {
    let mut d = stamped(Some(1.0), 0.5);
    d.retire(5.0);
    assert_eq!((d.lifetime, d.fade), (Some(1.0), 0.5));
}
