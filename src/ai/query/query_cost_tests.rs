//! What one environment query costs on the Dust2-sized bench level (#1892):
//! a 64-point donut with `trace` and `path` tests, the dear pair. The gate run
//! (`n = 5`) checks the query works at level scale and that its one flood stays
//! local; the timing is a release-build measurement run by hand, in
//! `dev::bench::query_cost_tests` (a sim module may not read a clock).

use glam::{Vec2, Vec3};

use super::{run, Anchor, Filter, Generator, Measure, QuerySpec, QueryWorld, Test};
use crate::components::ColliderShape;
use crate::navigation::{NavigationGraph, ALL_AREAS};
use crate::physics::PhysicsWorld;
use crate::scene::lighting::occlusion::bench::level::rooms;
use crate::scene::{Camera, ColliderComponent, Scene};

/// The bench level with a box collider on every block, baked at `cell` metres.
pub(crate) fn level(n: usize, cell: f32) -> (Scene, NavigationGraph, PhysicsWorld) {
    let mut scene = rooms(n);
    let ids: Vec<u32> = scene.world.ids_with_mesh();
    for id in ids {
        let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
        col.shape = ColliderShape::Box { size: Vec3::ONE };
        scene.world.set_collider(id, Some(col));
    }
    scene.nav_settings.grid_spacing = cell;
    let mut nav = NavigationGraph::new(0.0, 0.0, 0.0, 0.0, cell);
    nav.bake(&scene);
    let physics = PhysicsWorld::from_scene(&scene);
    (scene, nav, physics)
}

/// 4 rings × 16 around a target in room (2, 2), the bot in the next room.
fn spec(target: Anchor) -> QuerySpec {
    let filter = |measure| Test {
        measure,
        filter: Some(Filter::Wanted),
        score: None,
    };
    QuerySpec {
        generator: Generator::Donut {
            center: target.position,
            inner: 2.0,
            outer: 8.0,
            rings: 4,
            per_ring: 16,
        },
        project: 2.0,
        tests: vec![
            filter(Measure::Trace {
                from: target,
                from_height: 1.5,
                item_height: 1.5,
                mask: None,
                visible: true,
            }),
            filter(Measure::PathExists),
        ],
    }
}

pub(crate) fn query(scene: &Scene, nav: &NavigationGraph, physics: &PhysicsWorld) -> usize {
    let camera = Camera::new(Vec3::new(25.0, 1.5, 25.0), 0.0, 0.0);
    let world = QueryWorld {
        scene,
        nav,
        physics: Some(physics),
        view: &camera,
        screen: Vec2::new(1600.0, 900.0),
    };
    let target = Anchor::point(Vec3::new(25.0, 0.0, 25.0));
    let bot = Anchor::point(Vec3::new(35.0, 0.0, 24.0));
    run(&world, bot, &spec(target)).len()
}

/// The worst case for `path`: a bot 40 m from the target, and one survivor on
/// the upper storey, which no stair reaches, so the flood runs to its bound.
/// Eight spots on the ground round the target, then the one upstairs; the ones
/// the bot can reach. (At 1 m cells the doorways erode shut, so this wants the
/// 0.25 m navmesh.)
pub(crate) fn query_far(
    scene: &Scene,
    nav: &NavigationGraph,
    physics: &PhysicsWorld,
) -> Vec<super::Item> {
    let camera = Camera::new(Vec3::new(25.0, 1.5, 25.0), 0.0, 0.0);
    let world = QueryWorld {
        scene,
        nav,
        physics: Some(physics),
        view: &camera,
        screen: Vec2::new(1600.0, 900.0),
    };
    let target = Vec3::new(5.0, 0.0, 25.0);
    let mut points: Vec<Vec3> = (0..8)
        .map(|k| {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            target + 3.0 * Vec3::new(a.cos(), 0.0, a.sin())
        })
        .collect();
    points.push(target + Vec3::Y * 4.0);
    let spec = QuerySpec {
        generator: Generator::Points(points),
        project: 2.0,
        tests: vec![Test {
            measure: Measure::PathExists,
            filter: Some(Filter::Wanted),
            score: None,
        }],
    };
    let bot = Anchor::point(Vec3::new(45.0, 0.0, 25.0));
    run(&world, bot, &spec)
}

#[test]
fn a_far_bot_reaches_the_ground_spots_and_not_the_upper_storey() {
    let (scene, nav, physics) = level(5, 0.25);
    let found = query_far(&scene, &nav, &physics);
    assert!(!found.is_empty(), "the ground spots are reachable");
    assert!(found.iter().all(|i| i.position.y < 1.0), "{found:?}");
}

#[test]
fn a_query_on_the_bench_level_finds_spots_and_floods_locally() {
    let (scene, nav, physics) = level(5, 1.0);
    let found = query(&scene, &nav, &physics);
    assert!(found > 0 && found < 64, "some spots, not all: {found}");
    let flood = nav
        .flood(Vec3::new(35.0, 0.0, 24.0), 26.0, ALL_AREAS)
        .expect("on the navmesh");
    assert!(
        flood.reached() * 2 < nav.spans.len(),
        "{} of {}",
        flood.reached(),
        nav.spans.len()
    );
}
