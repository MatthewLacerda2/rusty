//! Review fixes to reflection placement (#1737): a small manual probe leaves its
//! room a probe, a corridor round a solid core is no cluttered room, and the probe
//! ceiling holds with clutter and manual probes.

use glam::Vec3;

use super::test_level::{block, plan_static};
use super::{plan_reflection_probes, ReflectionPlacement, Site};
use crate::scene::{ReflectionProbe, Scene};

/// A closed `2r × 2r` room under a roof 4 m up.
fn room(r: f32) -> Scene {
    let mut scene = Scene::new();
    block(&mut scene, Vec3::new(-r, -0.2, -r), Vec3::new(r, 0.0, r));
    block(&mut scene, Vec3::new(-r, 4.0, -r), Vec3::new(r, 4.2, r));
    scene
}

fn plan_with(scene: &Scene, params: &ReflectionPlacement, manual: &[ReflectionProbe]) -> usize {
    let site = Site::survey(scene, None, None, params.cell).expect("a site");
    plan_reflection_probes(scene, &site, params, manual).len()
}

#[test]
fn a_small_manual_probe_leaves_the_room_its_own() {
    let scene = room(5.0);
    let params = ReflectionPlacement::default();
    assert_eq!(plan_with(&scene, &params, &[]), 1);
    let small = ReflectionProbe::new(Vec3::new(0.0, 1.6, 0.0), Vec3::splat(1.0));
    assert_eq!(
        plan_with(&scene, &params, &[small]),
        1,
        "a corner box is not the room"
    );
    let whole = ReflectionProbe::new(Vec3::new(0.0, 2.0, 0.0), Vec3::new(5.0, 2.0, 5.0));
    assert_eq!(
        plan_with(&scene, &params, &[whole]),
        0,
        "the room is the manual probe's"
    );
}

/// A 3 m corridor round a 10 × 10 m solid core: the core is architecture, so the
/// corridor is probed as an empty room is, not as a hangar of crates.
#[test]
fn a_corridor_round_a_solid_core_is_not_clutter() {
    let mut scene = room(8.0);
    block(
        &mut scene,
        Vec3::new(-5.0, 0.0, -5.0),
        Vec3::new(5.0, 4.0, 5.0),
    );
    let plans = plan_static(&scene, &ReflectionPlacement::default());
    assert!((1..=4).contains(&plans.len()), "{} probes", plans.len());
}

/// A cluttered room under a tight ceiling, and manual probes already at it.
#[test]
fn the_probe_ceiling_holds_with_clutter_and_manual_probes() {
    let mut scene = room(6.0);
    for k in 0..12 {
        let (x, z) = (-4.5 + (k % 4) as f32 * 3.0, -4.5 + (k / 4) as f32 * 3.0);
        block(
            &mut scene,
            Vec3::new(x, 0.0, z),
            Vec3::new(x + 1.5, 1.5, z + 1.5),
        );
    }
    let tight = ReflectionPlacement {
        max_probes: 2,
        ..ReflectionPlacement::default()
    };
    assert!(
        plan_with(&scene, &ReflectionPlacement::default(), &[]) > 2,
        "cluttered"
    );
    assert!(plan_with(&scene, &tight, &[]) <= 2);
    let corner = ReflectionProbe::new(Vec3::new(-5.0, 1.6, -5.0), Vec3::splat(0.5));
    let full = [corner.clone(), corner];
    assert_eq!(
        plan_with(&scene, &tight, &full),
        0,
        "the manual probes fill it"
    );
}
