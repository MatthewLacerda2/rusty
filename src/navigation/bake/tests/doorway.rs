//! The doorway level the rebake and carving tests share (#456): a floor split by
//! a wall with a doorway, and whether a path still goes through it.

use super::super::super::test_support::{add_box, add_floor, ground, move_to};
use super::super::super::{NavBounds, NavigationGraph};
use crate::scene::Scene;
use glam::Vec3;

/// A 20 × 10 floor split by a wall at x = 10 with a doorway over z 3..8.
pub(super) fn doorway() -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 10.0));
    scene.nav_settings.agent_radius = 0.4;
    add_floor(&mut scene, -1.0, 21.0, -1.0, 11.0);
    add_box(
        &mut scene,
        Vec3::new(9.8, 0.0, -1.0),
        Vec3::new(10.2, 3.0, 3.0),
    );
    add_box(
        &mut scene,
        Vec3::new(9.8, 0.0, 8.0),
        Vec3::new(10.2, 3.0, 11.0),
    );
    scene
}

pub(super) fn through(g: &NavigationGraph) -> bool {
    g.find_path(ground(g, 3, 5), ground(g, 17, 5)).is_some()
}

pub(super) fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

/// The issue's corridor: moving a static box into a doorway closes the way
/// through, moving it out opens it again.
#[test]
fn moving_a_static_box_closes_and_opens_a_corridor() {
    let mut scene = doorway();
    let door = add_box(
        &mut scene,
        Vec3::new(14.0, 0.0, 0.5),
        Vec3::new(15.0, 2.0, 3.5),
    );
    let mut g = baked(&scene);
    assert!(through(&g), "the doorway is open");
    move_to(&mut scene, door, Vec3::new(10.0, 1.0, 5.5));
    g.sync(&scene);
    assert!(!through(&g), "the box blocks the doorway");
    move_to(&mut scene, door, Vec3::new(14.5, 1.0, 2.0));
    g.sync(&scene);
    assert!(through(&g), "the doorway is open again");
}
