//! Areas in path queries (#460): a costly strip is routed around when a cheap detour
//! exists and crossed when it doesn't, a masked-out area is never entered, the
//! smoothing never pulls a string across what A\* routed around, and a runtime cost
//! change re-routes without a rebake.

use glam::Vec3;

use super::super::test_support::{add_floor, add_modifier};
use super::super::{NavArea, NavBounds, NavPath, NavPathStatus, NavigationGraph, Rebake};
use crate::scene::Scene;

const MUD: u8 = 2;

/// A 20 × 10 floor with a mud strip across x 9..11 over z 0..`reach`.
fn strip(reach: f32, cost: f32) -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 10.0));
    scene.nav_settings.agent_radius = 0.0;
    scene.nav_settings.areas.push(NavArea {
        name: "Mud".into(),
        cost,
    });
    add_floor(&mut scene, -1.0, 21.0, -1.0, 11.0);
    let size = Vec3::new(3.0, 2.0, reach + 1.0);
    add_modifier(&mut scene, Vec3::new(10.0, 0.0, reach / 2.0), size, MUD);
    scene
}

fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

const FROM: Vec3 = Vec3::new(2.0, 0.0, 3.0);
const TO: Vec3 = Vec3::new(18.0, 0.0, 3.0);

/// Whether any leg of `path` walks through mud.
fn crosses_mud(g: &NavigationGraph, path: &NavPath) -> bool {
    let dry = !(1 << MUD);
    path.corners
        .windows(2)
        .any(|leg| g.raycast_masked(leg[0], leg[1], dry).is_none_or(|w| w.hit))
}

#[test]
fn a_costly_strip_is_routed_around_when_a_detour_exists() {
    let g = baked(&strip(7.0, 10.0));
    let path = g.calculate_path(FROM, TO);
    assert_eq!(path.status, NavPathStatus::Complete);
    assert!(
        path.corners.iter().any(|c| c.z >= 8.0),
        "{:?}",
        path.corners
    );
    assert!(
        !crosses_mud(&g, &path),
        "the smoothing keeps out of the mud: {:?}",
        path.corners
    );
    let cheap = baked(&strip(7.0, 1.0)).calculate_path(FROM, TO);
    assert_eq!(cheap.corners.len(), 2, "at cost 1 the mud is just floor");
}

#[test]
fn a_costly_strip_is_crossed_when_nothing_goes_around() {
    let g = baked(&strip(10.0, 10.0));
    let path = g.calculate_path(FROM, TO);
    assert_eq!(path.status, NavPathStatus::Complete);
    assert!(crosses_mud(&g, &path));
}

#[test]
fn a_masked_out_area_is_never_entered() {
    let g = baked(&strip(10.0, 1.0));
    let dry = !(1 << MUD);
    let path = g.calculate_path_masked(FROM, TO, dry);
    assert_eq!(
        path.status,
        NavPathStatus::Partial,
        "the mud cuts the field"
    );
    assert!(path.corners.iter().all(|c| c.x < 9.0), "{:?}", path.corners);
    let around = baked(&strip(7.0, 1.0)).calculate_path_masked(FROM, TO, dry);
    assert_eq!(around.status, NavPathStatus::Complete);
    assert!(!crosses_mud(&baked(&strip(7.0, 1.0)), &around));
    assert_eq!(
        g.sample_position_masked(Vec3::new(10.0, 0.0, 5.0), 0.6, dry),
        None
    );
}

#[test]
fn a_runtime_cost_change_reroutes_without_a_rebake() {
    let mut scene = strip(7.0, 1.0);
    let mut g = baked(&scene);
    assert_eq!(
        g.calculate_path(FROM, TO).corners.len(),
        2,
        "straight through"
    );
    let generation = g.bake_generation;
    assert_eq!(scene.nav_settings.set_area_cost("Mud", 10.0), Some(MUD));
    assert_eq!(g.sync(&scene), Rebake::Unchanged, "no span is rebaked");
    assert_ne!(g.bake_generation, generation, "cached agent paths go stale");
    assert!(!crosses_mud(&g, &g.calculate_path(FROM, TO)));
}
