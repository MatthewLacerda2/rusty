//! The filter tests (#1892, *Done when*): `trace` rejects points behind a wall,
//! `path = "exists"` rejects an unreachable roof, `dot` keeps the points behind a
//! target facing +Z, and `view` keeps the points outside the camera's frustum.

use glam::Vec3;

use super::test_world::{wall, Arena};
use super::{run, Anchor, Filter, Generator, Measure, QuerySpec, Test};

/// Run a points query with one filtering test; the surviving points, in order.
fn survivors(
    arena: &Arena,
    querier: Anchor,
    points: &[Vec3],
    measure: Measure,
    f: Filter,
) -> Vec<Vec3> {
    let spec = QuerySpec {
        generator: Generator::Points(points.to_vec()),
        project: 2.0,
        tests: vec![Test {
            measure,
            filter: Some(f),
            score: None,
        }],
    };
    let mut got: Vec<Vec3> = run(&arena.world(), querier, &spec)
        .into_iter()
        .map(|i| i.position)
        .collect();
    got.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z)));
    got
}

fn near(got: &[Vec3], want: &[Vec3]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (*a - *b).length() < 0.6)
}

#[test]
fn trace_rejects_a_point_behind_a_wall() {
    let mut arena = Arena::new(|s| {
        wall(s, (5.0, -3.0), (6.0, 3.0));
    });
    let target = arena.agent(Vec3::ZERO);
    let from = arena.anchor(target);
    let points = [Vec3::new(3.0, 0.0, 0.5), Vec3::new(9.0, 0.0, 0.5)];
    let trace = |visible| Measure::Trace {
        from,
        from_height: 1.5,
        item_height: 1.5,
        mask: None,
        visible,
    };
    let seen = survivors(&arena, from, &points, trace(true), Filter::Wanted);
    assert!(near(&seen, &points[..1]), "{seen:?}");
    let hidden = survivors(&arena, from, &points, trace(false), Filter::Wanted);
    assert!(near(&hidden, &points[1..]), "cover: {hidden:?}");
}

#[test]
fn path_exists_rejects_a_point_on_an_unreachable_roof() {
    let mut arena = Arena::new(|s| {
        rusty_navigation::test_support::add_box(
            s,
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(15.0, 5.0, 15.0),
        );
    });
    let querier = arena.agent(Vec3::ZERO);
    let roof = Vec3::new(12.5, 5.0, 12.5);
    assert!(
        arena.nav.sample_position(roof, 0.5).is_some(),
        "the roof is walkable"
    );
    let points = [Vec3::new(5.0, 0.0, 5.0), roof];
    let got = survivors(
        &arena,
        arena.anchor(querier),
        &points,
        Measure::PathExists,
        Filter::Wanted,
    );
    assert!(near(&got, &points[..1]), "{got:?}");
}

#[test]
fn dot_keeps_only_the_points_behind_a_target_facing_plus_z() {
    let mut arena = Arena::new(|_| {});
    let target = arena.agent(Vec3::ZERO);
    let from = arena.anchor(target);
    assert!((from.facing.expect("an entity faces") - Vec3::Z).length() < 1e-5);
    let points = [
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::new(0.5, 0.0, -5.0),
    ];
    let behind = Filter::Range {
        min: None,
        max: Some(-0.5),
    };
    let got = survivors(
        &arena,
        from,
        &points,
        Measure::Dot { from, flat: true },
        behind,
    );
    assert!(near(&got, &points[2..]), "{got:?}");
}

#[test]
fn view_keeps_only_the_points_outside_the_camera_frustum() {
    let mut arena = Arena::new(|_| {});
    let querier = arena.agent(Vec3::ZERO);
    // The camera stands at (-10, 1.5, 0) looking down +X.
    let ahead = Vec3::new(5.0, 0.0, 0.0);
    let behind = Vec3::new(-15.0, 0.0, 0.0);
    let aside = Vec3::new(-5.0, 0.0, 15.0);
    let points = [behind, aside, ahead];
    let view = |margin, inside| Measure::View {
        margin,
        item_height: 1.0,
        inside,
    };
    let q = arena.anchor(querier);
    let outside = survivors(&arena, q, &points, view(0.0, false), Filter::Wanted);
    assert!(near(&outside, &[behind, aside]), "{outside:?}");
    let inside = survivors(&arena, q, &points, view(0.0, true), Filter::Wanted);
    assert!(near(&inside, &[ahead]), "{inside:?}");
    // A wide margin takes the point off to the side into view.
    let wide = view(40.0, true);
    let got = survivors(&arena, q, &points, wide, Filter::Wanted);
    assert!(near(&got, &[aside, ahead]), "{got:?}");
}
