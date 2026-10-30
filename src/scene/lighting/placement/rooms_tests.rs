//! Tests for placing one reflection probe per room (#1584), on primitive levels.

use glam::Vec3;

use super::super::test_level::{arena, grid_of_rooms, WALL_HEIGHT};
use super::super::{
    plan_reflection_probes, static_scene_aabb, ReflectionPlacement, ReflectionPlan, EYE_HEIGHT,
};
use crate::scene::Scene;

fn plan(scene: &Scene) -> Vec<ReflectionPlan> {
    plan_reflection_probes(scene, None, &ReflectionPlacement::default())
}

/// The plans whose capture point stands within `x` (exclusive).
fn standing_in(plans: &[ReflectionPlan], x: (f32, f32)) -> Vec<ReflectionPlan> {
    let inside = |p: &&ReflectionPlan| p.position.x > x.0 && p.position.x < x.1;
    plans.iter().filter(inside).copied().collect()
}

#[test]
fn each_side_room_of_the_arena_gets_one_probe_fitted_to_its_walls() {
    let plans = plan(&arena());
    for (name, x) in [("red", (-22.0, -10.0)), ("blue", (10.0, 22.0))] {
        let room = standing_in(&plans, x);
        assert_eq!(room.len(), 1, "{name}: {room:?}");
        let p = room[0];
        // The box meets the walls (to within a cell and the doorway it takes a
        // share of), the floor and the roof.
        let near = |a: f32, b: f32, tol: f32| (a - b).abs() <= tol;
        assert!(
            near(p.box_min.x, x.0, 0.6) && near(p.box_max.x, x.1, 0.6),
            "{name}: {p:?}"
        );
        assert!(
            near(p.box_min.z, -6.0, 0.5) && near(p.box_max.z, 6.0, 0.5),
            "{name}: {p:?}"
        );
        assert_eq!((p.box_min.y, p.box_max.y), (0.0, WALL_HEIGHT), "{name}");
        assert!(near(p.position.x, (x.0 + x.1) * 0.5, 0.5) && near(p.position.z, 0.0, 0.5));
        assert_eq!(p.position.y, EYE_HEIGHT, "{name}: eye height");
    }
}

#[test]
fn the_open_hall_is_split_into_probes_no_wider_than_the_region() {
    let plans = plan(&arena());
    let hall = standing_in(&plans, (-10.0, 10.0));
    assert_eq!(hall.len(), 10, "20 × 52 m in ≤ 12 m tiles: 2 × 5");
    let sky = static_scene_aabb(&arena()).unwrap().1.y;
    for p in &hall {
        let size = p.box_max - p.box_min;
        assert!(size.x <= 12.5 && size.z <= 12.5, "{p:?}");
        assert!(
            p.box_min.x >= -11.0 && p.box_max.x <= 11.0,
            "inside the hall: {p:?}"
        );
        assert_eq!(
            p.box_max.y, sky,
            "open: the box reaches the top of the level"
        );
    }
}

#[test]
fn no_capture_point_is_inside_geometry() {
    let scene = arena();
    let plans = plan(&scene);
    for p in &plans {
        for id in scene.world.ids_with_collider() {
            let pose = scene.world.world_pose(id).unwrap();
            let (lo, hi) = scene.world.collider(id).unwrap().calculate_world_aabb(pose);
            let inside = p.position.cmpgt(lo).all() && p.position.cmplt(hi).all();
            assert!(!inside, "{p:?} is inside {lo:?}..{hi:?}");
        }
    }
}

#[test]
fn a_dust2_sized_level_gets_a_probe_per_room() {
    // 10 × 10 rooms of 20 m: 200 m across, Dust2's scale. Each 20 m room is split
    // in 2 × 2 by the 12 m region.
    let scene = grid_of_rooms(10, 20.0);
    let plans = plan(&scene);
    assert_eq!(plans.len(), 400);
    let first = plans[0];
    assert!(
        first.box_max.x <= 11.0 && first.box_max.z <= 11.0,
        "{first:?}"
    );
    assert!(plans.iter().all(|p| p.position.is_finite()));
    let again = plan(&scene);
    assert_eq!(plans, again, "deterministic");
}

#[test]
fn an_area_too_narrow_for_a_core_gets_its_own_probes() {
    // A 1.5 m wide, 20 m long walkway alone: narrower than a doorway everywhere,
    // so it holds no room core, and still gets probes, tiled by the 12 m region.
    let mut scene = Scene::new();
    let block = super::super::test_level::block;
    block(
        &mut scene,
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(1.5, 0.0, 20.0),
    );
    let plans = plan(&scene);
    assert_eq!(plans.len(), 2, "{plans:?}");
    for p in &plans {
        assert!(p.box_min.x >= 0.0 && p.box_max.x <= 1.5, "{p:?}");
        assert!(p.box_max.z - p.box_min.z <= 12.5, "{p:?}");
    }
}
