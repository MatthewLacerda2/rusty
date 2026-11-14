//! Reflection probes only on reachable floor (#1737).

use glam::Vec3;

use super::{plan_on_nav, plan_on_nav_from, site_on_nav_from};
use rusty_lighting::placement::test_sites::{indoor_house, roofed_house, yard};
use rusty_lighting::placement::ReflectionPlan;

/// No plan on the roof, and one in the room.
fn assert_in_the_room(plans: &[ReflectionPlan]) {
    assert!(!plans.is_empty());
    for p in plans {
        assert!(p.position.y < 3.0, "a probe on the roof: {p:?}");
    }
    let inside = |p: &&ReflectionPlan| p.position.x.abs() < 5.0 && p.position.z.abs() < 5.0;
    assert!(
        plans.iter().any(|p| inside(&p)),
        "the room has its own probe"
    );
}

/// An indoor level whose roof is larger than its room: with no actor the room is
/// still the main floor (the roof is an elevated top), and with one in the room
/// the roof gets nothing either, light probes included.
#[test]
fn an_indoor_roof_gets_no_probe_with_or_without_an_actor() {
    let scene = indoor_house();
    for starts in [&[][..], &[Vec3::new(0.0, 0.1, 0.0)]] {
        assert_in_the_room(&plan_on_nav_from(&scene, starts));
        let light = site_on_nav_from(&scene, starts).plan_light_probes(4.0, &[]);
        assert!(!light.is_empty());
        assert!(light.iter().all(|p| p.y < 4.0), "a light probe on the roof");
    }
}

#[test]
fn an_unreachable_roof_gets_no_probe() {
    let mut scene = yard();
    roofed_house(&mut scene);
    assert_in_the_room(&plan_on_nav(&scene));
}

/// The only actor is a sentry on the roof: the room, the main floor, still gets
/// its own probe.
#[test]
fn a_sentry_on_the_roof_keeps_the_room_probed() {
    let scene = indoor_house();
    let plans = plan_on_nav_from(&scene, &[Vec3::new(0.0, 4.4, 0.0)]);
    let inside = |p: &ReflectionPlan| {
        p.position.x.abs() < 5.0 && p.position.z.abs() < 5.0 && p.position.y < 3.0
    };
    assert!(
        plans.iter().any(inside),
        "the room has its own probe: {plans:?}"
    );
}

/// A yard round a roofed house: its open-air probe captures from under the sky,
/// not from the doorway nearest the yard's centroid, which is inside the house.
#[test]
fn a_yard_round_a_house_captures_outdoors() {
    let mut scene = yard();
    roofed_house(&mut scene);
    let plans = plan_on_nav(&scene);
    let open: Vec<_> = plans.iter().filter(|p| !p.box_projection).collect();
    assert_eq!(open.len(), 1, "{plans:?}");
    let at = open[0].position;
    assert!(
        at.x.abs() > 7.0 || at.z.abs() > 7.0,
        "under the roof at {at}"
    );
}
