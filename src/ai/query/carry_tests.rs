//! The time-sliced query across rebakes (#2116): a carving obstacle moving every
//! few ticks far from the search never stalls the queue (the held flood is
//! carried, not restarted), and one moving where the search has been gives the
//! answer of the navmesh it ends on.

use glam::{Vec2, Vec3};

use super::queue::Status;
use super::test_world::wall;
use super::{pick, run, Anchor, Generator, Measure, Progress, QueryQueue, QuerySpec, QueryWorld};
use super::{Curve, Item, RunMode, Score, Test};
use crate::core::random::Random;
use crate::navigation::test_support::{add_floor, add_obstacle, move_to};
use crate::navigation::{tick_obstacles, NavBounds, NavigationGraph, Rebake};
use crate::scene::{Camera, Scene};

const FROM: Vec3 = Vec3::new(-40.0, 0.0, 5.0);
/// A 90 × 20 m floor, a wall at x = −32 with two doorways (z 3..7 and −7..−3),
/// and a carving obstacle at `at`.
fn level(at: Vec3) -> (Scene, NavigationGraph, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 1.0;
    scene.nav_settings.bounds = Some(NavBounds::new(-45.0, 45.0, -10.0, 10.0));
    add_floor(&mut scene, -46.0, 46.0, -11.0, 11.0);
    for (z0, z1) in [(-11.0, -7.0), (-3.0, 3.0), (7.0, 11.0)] {
        wall(&mut scene, (-32.5, z0), (-31.5, z1));
    }
    let obstacle = add_obstacle(&mut scene, at, Vec3::new(1.0, 2.0, 4.0), true);
    tick_obstacles(&mut scene, 1.0 / 60.0);
    let nav = NavigationGraph::from_scene(&scene);
    (scene, nav, obstacle)
}

/// Reachable donut spots (an unreachable one has no length), nearer by path better.
fn spec() -> QuerySpec {
    let test = |measure, score| Test {
        measure,
        filter: None,
        score,
    };
    let nearer = Some(Score {
        curve: Curve::Inverse,
        weight: 1.0,
    });
    QuerySpec {
        generator: Generator::Donut {
            center: Vec3::new(-22.0, 0.0, 0.0),
            inner: 2.0,
            outer: 8.0,
            rings: 2,
            per_ring: 12,
        },
        project: 2.0,
        tests: vec![test(Measure::PathLength, nearer)],
    }
}

/// What a query reads: no physics (no `trace` test) and any camera.
fn world<'a>(scene: &'a Scene, nav: &'a NavigationGraph, view: &'a Camera) -> QueryWorld<'a> {
    let (physics, screen) = (None, Vec2::new(1600.0, 900.0));
    QueryWorld {
        scene,
        nav,
        physics,
        view,
        screen,
    }
}

/// Request `n` queries at `budget` expansions a tick and serve them, moving the
/// obstacle to `path(tick)` every third tick while it gives a spot: each answer
/// and the tick it came, and the run's final scene and navmesh.
fn serve(
    start: Vec3,
    path: impl Fn(u32) -> Option<Vec3>,
    n: usize,
    budget: u32,
) -> (Vec<(Option<Item>, u32)>, Scene, NavigationGraph) {
    let (mut scene, mut nav, obstacle) = level(start);
    let camera = Camera::new(FROM, 0.0, 0.0);
    let (mut queue, mut rng) = (QueryQueue::default(), Random::new(3));
    queue.set_iterations_per_frame(budget);
    let tickets: Vec<_> = (0..n)
        .map(|_| {
            let w = world(&scene, &nav, &camera);
            let progress = Progress::start(&w, Anchor::point(FROM), spec());
            queue.request(0, progress, RunMode::Best, &mut rng)
        })
        .collect();
    let mut answers = vec![None; n];
    for tick in 1..2_000 {
        if let Some(at) = path(tick).filter(|_| tick % 3 == 0) {
            move_to(&mut scene, obstacle, at);
            tick_obstacles(&mut scene, 1.0 / 60.0);
            assert!(matches!(nav.sync(&scene), Rebake::Incremental(_)));
        }
        queue.serve(&nav, &mut rng);
        for (answer, &t) in answers.iter_mut().zip(&tickets) {
            if let Status::Done(item) = queue.poll(t) {
                *answer = Some((item, tick));
            }
        }
        if answers.iter().all(Option::is_some) {
            return (answers.into_iter().flatten().collect(), scene, nav);
        }
    }
    panic!("every query answers: {answers:?}");
}

/// [`run`]'s `best` pick on `scene` and `nav`.
fn synchronous(scene: &Scene, nav: &NavigationGraph) -> Option<Item> {
    let camera = Camera::new(FROM, 0.0, 0.0);
    let ranked = run(&world(scene, nav, &camera), Anchor::point(FROM), &spec());
    pick(&ranked, RunMode::Best, &mut Random::new(3))
}

const FAR: Vec3 = Vec3::new(30.0, 1.0, 0.0);

#[test]
fn an_obstacle_rebaking_far_away_never_stalls_a_held_query() {
    let (still, ..) = serve(FAR, |_| None, 1, 32);
    assert!(still[0].1 > 6, "a long search: {} ticks", still[0].1);
    let wander = |tick: u32| Some(FAR + Vec3::X * ((tick / 3 % 6) as f32));
    let (answers, scene, nav) = serve(FAR, wander, 3, 32);
    let want = synchronous(&scene, &nav);
    assert!(want.is_some(), "the donut is reachable");
    for (i, &(item, tick)) in answers.iter().enumerate() {
        assert_eq!(item, want, "query {i}: the final navmesh's answer");
        assert!(
            tick <= 4 * still[0].1 * (i as u32 + 1),
            "query {i}: tick {tick}"
        );
    }
}

#[test]
fn an_obstacle_rebaking_where_the_search_went_gives_the_final_navmeshs_answer() {
    let open = {
        let (scene, nav, _) = level(FAR);
        synchronous(&scene, &nav)
    };
    // Into the upper doorway, then rocking in it until tick 12.
    let door = |tick: u32| Vec3::new(-32.0 + (tick % 2) as f32 * 0.2, 1.0, 5.0);
    let shut = move |tick: u32| (tick <= 12).then(|| door(tick));
    let (answers, scene, nav) = serve(FAR, shut, 1, 32);
    let want = synchronous(&scene, &nav);
    assert_eq!(answers[0].0, want, "the shut door's answer");
    assert_ne!(want, open, "the door moved the answer");
}
