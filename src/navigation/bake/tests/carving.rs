//! `NavMeshObstacle` carving (#456): a carving obstacle cuts the navmesh where it
//! stands, re-opens it when removed, leaves other floors alone, and waits to be
//! stationary when asked to.

use super::super::super::test_support::{add_box, add_obstacle, assert_same_mesh, move_to};
use super::super::super::{tick_obstacles, Rebake};
use super::doorway::{baked, doorway, through};
use crate::scene::Scene;
use glam::Vec3;

/// A door-sized obstacle in the doorway.
fn door(scene: &mut Scene, carve: bool) -> u32 {
    add_obstacle(
        scene,
        Vec3::new(10.0, 1.0, 5.5),
        Vec3::new(0.4, 2.0, 5.0),
        carve,
    )
}

#[test]
fn a_carving_obstacle_blocks_the_path_and_reopens_when_removed() {
    let mut scene = doorway();
    let mut g = baked(&scene);
    assert!(through(&g));
    let id = door(&mut scene, true);
    assert!(matches!(g.sync(&scene), Rebake::Incremental(_)));
    assert!(!through(&g), "the closed door cuts the doorway");
    assert_same_mesh(&g, &baked(&scene));
    scene.world.set_nav_obstacle(id, None);
    g.sync(&scene);
    assert!(through(&g), "removing the door re-opens it");
    assert_same_mesh(&g, &baked(&scene));
}

#[test]
fn a_non_carving_obstacle_leaves_the_navmesh_alone() {
    let mut scene = doorway();
    let mut g = baked(&scene);
    door(&mut scene, false);
    assert_eq!(g.sync(&scene), Rebake::Unchanged);
    assert!(through(&g));
}

#[test]
fn carving_cuts_the_floor_it_stands_on_not_the_deck_above() {
    let mut scene = doorway();
    // A deck at y = 3 over x 2..8, z 2..8; the obstacle stands under it.
    add_box(
        &mut scene,
        Vec3::new(2.0, 2.8, 2.0),
        Vec3::new(8.0, 3.0, 8.0),
    );
    add_obstacle(&mut scene, Vec3::new(5.0, 0.5, 5.0), Vec3::ONE, true);
    let g = baked(&scene);
    let ys: Vec<f32> = g.spans_at(5, 5).iter().map(|s| s.y).collect();
    assert_eq!(
        ys,
        vec![3.0],
        "the ground under the crate is cut, the deck kept"
    );
    assert_eq!(
        g.spans_at(2, 5).len(),
        1,
        "erosion pulls the ground back too"
    );
}

#[test]
fn carve_only_stationary_waits_for_the_obstacle_to_stand_still() {
    let mut scene = doorway();
    let id = door(&mut scene, true);
    if let Some(mut o) = scene.world.nav_obstacle_mut(id) {
        o.carve_only_stationary = true;
    }
    let dt = 0.1;
    let mut g = baked(&scene);
    tick_obstacles(&mut scene, dt);
    g.sync(&scene);
    assert!(
        !through(&g),
        "standing in the doorway from the start: carves"
    );
    // Slide it out of the way: while moving, it carves nothing.
    for step in 1..=5 {
        let z = 5.5 + step as f32;
        move_to(&mut scene, id, Vec3::new(10.0, 1.0, z));
        tick_obstacles(&mut scene, dt);
        g.sync(&scene);
        assert!(through(&g), "moving (step {step}): no hole");
    }
    // Back into the doorway; it carves once still for `time_to_stationary`.
    move_to(&mut scene, id, Vec3::new(10.0, 1.0, 5.5));
    for _ in 0..4 {
        tick_obstacles(&mut scene, dt);
        g.sync(&scene);
        assert!(through(&g), "not yet stationary for 0.5 s");
    }
    tick_obstacles(&mut scene, dt);
    tick_obstacles(&mut scene, dt);
    g.sync(&scene);
    assert!(!through(&g), "stationary: the door carves again");
    assert_same_mesh(&g, &baked(&scene));
}
