//! What bounds reflection-probe placement (#1584): the region floor, the probe
//! ceiling and the floor plan's cell cap; triggers are not walls; a capture point
//! on a stair stands over its own step.

use glam::{Vec2, Vec3};

use super::floor::{FloorCell, FloorGrid, MAX_FLOOR_CELLS};
use super::room_probes::plan_rooms;
use super::test_level::{block, grid_of_rooms};
use super::{plan_reflection_probes, ReflectionPlacement, EYE_HEIGHT, MIN_REGION};
use crate::scene::Scene;

fn placed(scene: &Scene, region: f32, max_probes: usize) -> usize {
    let params = ReflectionPlacement {
        region,
        max_probes,
        ..ReflectionPlacement::default()
    };
    plan_reflection_probes(scene, None, &params).len()
}

/// A 20 × 20 m open floor.
fn floor() -> Scene {
    let mut scene = Scene::new();
    block(
        &mut scene,
        Vec3::new(-10.0, -0.2, -10.0),
        Vec3::new(10.0, 0.0, 10.0),
    );
    scene
}

#[test]
fn a_region_under_the_minimum_places_as_the_minimum_does() {
    let scene = floor();
    assert_eq!(placed(&scene, 0.0, 4096), placed(&scene, MIN_REGION, 4096));
    assert_eq!(placed(&scene, MIN_REGION, 4096), 100, "20 m in 2 m tiles");
}

#[test]
fn the_probe_ceiling_widens_the_tiles() {
    // The Dust2-sized grid: 400 probes at 12 m, under the default ceiling.
    let scene = grid_of_rooms(10, 20.0);
    assert_eq!(placed(&scene, 12.0, 512), 400);
    let capped = placed(&scene, 12.0, 150);
    assert!((1..=150).contains(&capped), "{capped}");
}

#[test]
fn a_huge_level_is_rasterized_at_a_coarser_cell() {
    let mut scene = Scene::new();
    let half = 5000.0;
    block(
        &mut scene,
        Vec3::new(-half, -0.2, -half),
        Vec3::new(half, 0.0, half),
    );
    let grid = FloorGrid::from_static(&scene, 0.5).unwrap();
    assert!(grid.width * grid.depth <= MAX_FLOOR_CELLS);
    assert!(grid.cell > 0.5);
}

#[test]
fn a_trigger_is_not_a_wall() {
    let params = ReflectionPlacement::default();
    let walled = |trigger: bool| {
        let mut scene = floor();
        block(
            &mut scene,
            Vec3::new(-0.2, 0.0, -10.0),
            Vec3::new(0.2, 4.0, 10.0),
        );
        let id = *scene.world.ids_with_collider().last().unwrap();
        scene.world.collider_mut(id).unwrap().is_trigger = trigger;
        plan_reflection_probes(&scene, None, &params)
    };
    let open = plan_reflection_probes(&floor(), None, &params);
    assert_eq!(walled(true), open);
    assert_ne!(walled(false), open, "a solid wall splits the floor");
}

#[test]
fn a_capture_point_on_a_stair_stands_over_its_own_step() {
    // 9 × 3 cells of 1 m: low-ceilinged floor either side of a step 3 m up, open
    // above. The tile's median ceiling (2 m) is under the step it stands on.
    let cells = (0..27)
        .map(|i| match i % 9 {
            4 => FloorCell {
                floor: 3.0,
                ceiling: 10.0,
            },
            _ => FloorCell {
                floor: 0.0,
                ceiling: 2.0,
            },
        })
        .map(Some)
        .collect();
    let grid = FloorGrid {
        origin: Vec2::ZERO,
        cell: 1.0,
        width: 9,
        depth: 3,
        cells,
        inset: 0.0,
    };
    let plans = plan_rooms(&grid, &[Some(0); 27], 1, 20.0, 50.0);
    let p = plans[0];
    assert_eq!(p.position.y, 3.0 + EYE_HEIGHT, "{p:?}");
    assert!(p.box_max.y > p.position.y, "the box holds it: {p:?}");
}
