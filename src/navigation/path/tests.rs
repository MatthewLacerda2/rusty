//! Path queries (#458) on baked primitive-box rooms.

use super::super::test_support::{add_box, add_floor, bake_pinned};
use super::super::NavigationGraph;
use super::NavPathStatus;
use crate::scene::Scene;
use glam::Vec3;

/// A floor over the whole grid, with a tall wall strip at x = 5 over `z0..z1`.
fn room(wall_z: Option<(f32, f32)>, radius: f32) -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    if let Some((z0, z1)) = wall_z {
        add_box(
            &mut scene,
            Vec3::new(4.75, 0.0, z0),
            Vec3::new(5.25, 3.0, z1),
        );
    }
    scene.nav_settings.agent_radius = radius;
    bake_pinned(&mut scene)
}

fn v(x: f32, z: f32) -> Vec3 {
    Vec3::new(x, 0.0, z)
}

#[test]
fn open_room_path_is_one_straight_run() {
    let g = room(None, 0.5);
    let path = g.calculate_path(v(2.0, 2.0), v(8.0, 7.0));
    assert_eq!(path.status, NavPathStatus::Complete);
    assert_eq!(path.corners, vec![v(2.0, 2.0), v(8.0, 7.0)], "no staircase");
    assert!((path.length() - 61f32.sqrt()).abs() < 1e-4);
    assert_eq!(g.get_next_path_step(v(2.0, 2.0), v(8.0, 7.0)), v(8.0, 7.0));
}

#[test]
fn path_around_a_wall_turns_only_at_its_end_and_cuts_across() {
    let g = room(Some((-5.0, 6.0)), 0.5);
    let path = g.calculate_path(v(2.0, 2.0), v(8.0, 2.0));
    assert_eq!(path.status, NavPathStatus::Complete);
    let n = path.corners.len();
    assert!(
        (3..=5).contains(&n),
        "turns round the wall end: {:?}",
        path.corners
    );
    assert_eq!(
        (path.corners[0], path.corners[n - 1]),
        (v(2.0, 2.0), v(8.0, 2.0))
    );
    for leg in path.corners.windows(2) {
        let walk = g.raycast(leg[0], leg[1]).expect("on the mesh");
        assert!(!walk.hit, "leg {leg:?} crosses unwalkable ground");
    }
    let staircase = g.path_between(v(2.0, 2.0), v(8.0, 2.0)).expect("a path");
    let cells: Vec<Vec3> = staircase.iter().map(|&s| g.span_world(s)).collect();
    assert!(
        path.length() < super::path_length(&cells),
        "shorter than the cell path"
    );
}

#[test]
fn raycast_stops_at_the_eroded_edge_of_a_wall() {
    let g = room(Some((-5.0, 15.0)), 1.0);
    assert!(
        !g.is_walkable(4, 5) && g.is_walkable(3, 5),
        "erosion left x = 3"
    );
    let walk = g.raycast(v(2.0, 5.0), v(8.0, 5.0)).expect("on the mesh");
    assert!(walk.hit);
    assert!(
        (walk.position.x - 3.5).abs() < 0.01,
        "stops at {}",
        walk.position
    );
    assert_eq!(
        g.world_to_grid(walk.position),
        (3, 5),
        "inside the last cell"
    );
    let clear = g.raycast(v(2.0, 2.0), v(3.0, 8.0)).expect("on the mesh");
    assert!(!clear.hit);
    assert_eq!(clear.position, v(3.0, 8.0));
}

#[test]
fn raycast_from_off_the_mesh_is_blocked_at_once() {
    let g = room(Some((-5.0, 15.0)), 1.0);
    let walk = g.raycast(v(5.0, 5.0), v(2.0, 5.0)).expect("a mesh nearby");
    assert!(walk.hit);
    assert_eq!(walk.position, v(5.0, 5.0));
}

#[test]
fn sample_position_snaps_onto_the_nearest_floor() {
    let g = room(Some((-5.0, 15.0)), 1.0);
    let above = g.sample_position(Vec3::new(2.0, 1.5, 2.0), 2.0);
    assert_eq!(above, Some(v(2.0, 2.0)), "straight down onto the floor");
    let p = g
        .sample_position(v(4.6, 5.0), 2.0)
        .expect("the eroded band's edge");
    assert!((p.x - 3.5).abs() < 0.01 && p.z == 5.0 && p.y == 0.0, "{p}");
    assert_eq!(g.world_to_grid(p), (3, 5));
    assert_eq!(g.sample_position(v(4.6, 5.0), 1.0), None, "out of reach");
    assert_eq!(g.sample_position(v(2.0, 2.0), f32::NAN), None);
}

#[test]
fn sample_position_picks_the_nearer_of_stacked_floors() {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    add_box(
        &mut scene,
        Vec3::new(3.0, 2.9, 3.0),
        Vec3::new(8.0, 3.0, 8.0),
    );
    scene.nav_settings.agent_radius = 0.0;
    let g = bake_pinned(&mut scene);
    let deck = g.sample_position(Vec3::new(5.0, 3.4, 5.0), 1.0);
    assert_eq!(deck, Some(Vec3::new(5.0, 3.0, 5.0)));
    let ground = g.sample_position(Vec3::new(5.0, 0.4, 5.0), 1.0);
    assert_eq!(ground, Some(v(5.0, 5.0)));
}

#[test]
fn unreachable_target_gives_a_partial_path_to_the_nearest_point() {
    let g = room(Some((-5.0, 15.0)), 1.0);
    let path = g.calculate_path(v(2.0, 5.0), v(8.0, 5.0));
    assert_eq!(path.status, NavPathStatus::Partial);
    let end = *path.corners.last().expect("corners");
    assert_eq!(end, v(3.0, 5.0), "the reachable span nearest the target");
}

#[test]
fn no_navmesh_means_an_invalid_path() {
    let g = bake_pinned(&mut Scene::new());
    let path = g.calculate_path(v(2.0, 2.0), v(8.0, 2.0));
    assert_eq!(path.status, NavPathStatus::Invalid);
    assert!(path.corners.is_empty() && path.length() == 0.0);
    assert_eq!(g.get_next_path_step(v(2.0, 2.0), v(8.0, 2.0)), v(8.0, 2.0));
}
