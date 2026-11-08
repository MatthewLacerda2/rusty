//! What a query's places mean (#1892, the review of PR #1921): an entity
//! context stands on its feet, a level grouped under one parent with the Player
//! keeps its walls, and a scored yes/no test favours the answer it asks for.

use glam::Vec3;

use super::test_world::{wall, Arena};
use super::{run, Anchor, Curve, Filter, Generator, Measure, QuerySpec, Score, Test};
use crate::components::CharacterControllerComponent;
use crate::scene::Scene;

/// A Player: a 2 m `CharacterController` whose Transform (its middle) is at `at`.
fn player(scene: &mut Scene, at: Vec3) -> u32 {
    let id = scene.add_entity("Player".to_string());
    scene.world.transform_mut(id).expect("new").position = at;
    let cc = CharacterControllerComponent {
        height: 2.0,
        radius: 0.5,
        ..Default::default()
    };
    scene.world.set_character_controller(id, Some(cc));
    id
}

fn best(arena: &Arena, q: Anchor, points: &[Vec3], test: Test) -> Vec3 {
    let spec = QuerySpec {
        generator: Generator::Points(points.to_vec()),
        project: 2.0,
        tests: vec![test],
    };
    run(&arena.world(), q, &spec)[0].position
}

#[test]
fn an_entity_context_stands_on_its_feet() {
    let mut scene = Scene::new();
    let p = player(&mut scene, Vec3::new(1.0, 1.0, 2.0));
    let feet = Anchor::of_entity(&scene, p).position;
    assert!(
        (feet - Vec3::new(1.0, 0.0, 2.0)).length() < 1e-5,
        "{feet:?}"
    );
    let crate_top = crate::navigation::test_support::add_box(
        &mut scene,
        Vec3::new(4.0, 0.5, 4.0),
        Vec3::new(6.0, 1.5, 6.0),
    );
    let bottom = Anchor::of_entity(&scene, crate_top).position;
    assert!(
        (bottom - Vec3::new(5.0, 0.5, 5.0)).length() < 1e-5,
        "{bottom:?}"
    );
}

/// A trace from the Player past a wall at x 5..6, both under one empty "Level".
fn grouped() -> (Arena, Anchor, [Vec3; 2]) {
    let mut id = 0;
    let arena = Arena::new(|s| {
        let level = s.add_entity("Level".to_string());
        id = player(s, Vec3::new(0.0, 1.0, 0.0));
        let w = wall(s, (5.0, -3.0), (6.0, 3.0));
        s.set_parent(id, Some(level)).expect("parent");
        s.set_parent(w, Some(level)).expect("parent");
    });
    let from = arena.anchor(id);
    (
        arena,
        from,
        [Vec3::new(3.0, 0.0, 0.5), Vec3::new(9.0, 0.0, 0.5)],
    )
}

fn trace(from: Anchor, visible: bool) -> Measure {
    Measure::Trace {
        from,
        from_height: 1.5,
        item_height: 1.5,
        mask: None,
        visible,
    }
}

#[test]
fn walls_grouped_with_the_player_still_block_its_sight() {
    let (arena, from, points) = grouped();
    let filter = |visible| Test {
        measure: trace(from, visible),
        filter: Some(Filter::Wanted),
        score: None,
    };
    let spec = |t| QuerySpec {
        generator: Generator::Points(points.to_vec()),
        project: 2.0,
        tests: vec![t],
    };
    let cover = run(&arena.world(), from, &spec(filter(false)));
    assert_eq!(cover.len(), 1, "{cover:?}");
    assert!((cover[0].position.x - 9.0).abs() < 0.6, "{cover:?}");
    let seen = run(&arena.world(), from, &spec(filter(true)));
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert!((seen[0].position.x - 3.0).abs() < 0.6, "{seen:?}");
}

#[test]
fn a_scored_yes_no_test_favours_the_answer_it_asks_for() {
    let scored = |measure| Test {
        measure,
        filter: None,
        score: Some(Score {
            curve: Curve::Linear,
            weight: 1.0,
        }),
    };
    let (arena, from, points) = grouped();
    let cover = best(&arena, from, &points, scored(trace(from, false)));
    assert!((cover.x - 9.0).abs() < 0.6, "cover ranks first: {cover:?}");
    let open = best(&arena, from, &points, scored(trace(from, true)));
    assert!((open.x - 3.0).abs() < 0.6, "in sight ranks first: {open:?}");
    // The camera stands at (-10, 1.5, 0) looking down +X.
    let view = |inside| Measure::View {
        margin: 0.0,
        item_height: 1.0,
        inside,
    };
    let points = [Vec3::new(2.0, 0.0, 3.0), Vec3::new(-15.0, 0.0, 0.0)];
    let out = best(&arena, from, &points, scored(view(false)));
    assert!(
        (out.x + 15.0).abs() < 0.6,
        "out of view ranks first: {out:?}"
    );
    let seen = best(&arena, from, &points, scored(view(true)));
    assert!((seen.x - 2.0).abs() < 0.6, "in view ranks first: {seen:?}");
}
