//! The spread and determinism tests (#1892, *Done when*): six agents asking the
//! same `separation` query around one target end up apart, and the same seed
//! always picks the same random spot.

use glam::Vec3;

use super::test_world::Arena;
use super::{pick, run, Curve, Filter, Generator, Measure, QuerySpec, RunMode, Score, Test};
use rusty_core::random::Random;

/// Six agents' query around `target`: reachable, near the asker, and (when
/// `spread`) away from the others' claims.
fn spec(target: Vec3, spread: bool) -> QuerySpec {
    let mut tests = vec![
        Test {
            measure: Measure::PathExists,
            filter: Some(Filter::Wanted),
            score: None,
        },
        Test {
            measure: Measure::Distance {
                to: super::Anchor::point(Vec3::new(-12.0, 0.0, 0.0)),
                flat: true,
            },
            filter: None,
            score: Some(Score {
                curve: Curve::Inverse,
                weight: 0.5,
            }),
        },
    ];
    if spread {
        tests.push(Test {
            measure: Measure::Separation { radius: 3.0 },
            filter: None,
            score: Some(Score {
                curve: Curve::Linear,
                weight: 2.0,
            }),
        });
    }
    QuerySpec {
        generator: Generator::Donut {
            center: target,
            inner: 3.0,
            outer: 6.0,
            rings: 2,
            per_ring: 16,
        },
        project: 2.0,
        tests,
    }
}

/// Each agent in turn runs the query and heads for its pick (the way bots do in
/// one tick); then they all walk there. Their ends.
fn settle(spread: bool) -> (Arena, Vec<Vec3>) {
    let mut arena = Arena::new(|_| {});
    let target = Vec3::ZERO;
    let agents: Vec<u32> = (0..6)
        .map(|i| arena.agent(Vec3::new(-12.0, 0.0, i as f32 * 0.3)))
        .collect();
    let spec = spec(target, spread);
    let mut ends = Vec::new();
    for &id in &agents {
        let ranked = run(&arena.world(), arena.anchor(id), &spec);
        let got = pick(&ranked, RunMode::Best, &mut Random::new(1)).expect("a spot");
        arena.scene.world.nav_agent_mut(id).expect("agent").target = got.position;
        ends.push(got.position);
    }
    for (&id, &end) in agents.iter().zip(&ends) {
        arena.scene.world.transform_mut(id).expect("agent").position = end;
    }
    (arena, ends)
}

fn closest_pair(ends: &[Vec3]) -> f32 {
    let mut least = f32::INFINITY;
    for (i, a) in ends.iter().enumerate() {
        for b in &ends[i + 1..] {
            least = least.min(a.distance(*b));
        }
    }
    least
}

#[test]
fn separation_spreads_six_agents_around_one_target() {
    let (arena, ends) = settle(true);
    let least = closest_pair(&ends);
    assert!(least >= 2.0, "closest pair {least} m: {ends:?}");
    for end in &ends {
        assert!(
            arena.nav.path_between(*end, Vec3::ZERO).is_some(),
            "{end:?} reaches the target"
        );
    }
    let (_, bunched) = settle(false);
    assert!(
        closest_pair(&bunched) < 1.0,
        "without separation they bunch: {bunched:?}"
    );
}

#[test]
fn the_same_seed_picks_the_same_random_spot() {
    let arena = Arena::new(|_| {});
    let querier = super::Anchor::point(Vec3::new(-12.0, 0.0, 0.0));
    let spec = spec(Vec3::ZERO, false);
    let choose = |seed: u64| {
        let ranked = run(&arena.world(), querier, &spec);
        pick(&ranked, RunMode::RandomBest25, &mut Random::new(seed))
            .expect("a spot")
            .position
    };
    assert_eq!(choose(42), choose(42));
    let first = choose(0);
    assert!(
        (1..50).any(|seed| choose(seed) != first),
        "another seed can pick another spot"
    );
}
