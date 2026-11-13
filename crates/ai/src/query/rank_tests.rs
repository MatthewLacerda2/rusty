//! Scoring and picking (#1892): normalization over survivors, the equal-values
//! rule, curves and weights, tie order, and how the random modes draw.

use glam::Vec3;

use super::rank::{pick, rank};
use super::spec::{Curve, Generator, Measure, QuerySpec, RunMode, Score, Test};
use super::Item;
use rusty_core::random::Random;

fn spec(scores: &[Score]) -> QuerySpec {
    let tests = scores
        .iter()
        .map(|&s| Test {
            measure: Measure::PathExists,
            filter: None,
            score: Some(s),
        })
        .collect();
    QuerySpec {
        generator: Generator::Points(Vec::new()),
        project: 1.0,
        tests,
    }
}

fn items(n: usize) -> Vec<Vec3> {
    (0..n).map(|i| Vec3::X * i as f32).collect()
}

fn score(curve: Curve, weight: f32) -> Score {
    Score { curve, weight }
}

#[test]
fn raw_values_normalize_over_the_survivors_and_shape_by_curve() {
    let raw = vec![vec![10.0, 99.0, 20.0, 30.0]];
    let alive = [0, 2, 3]; // item 1 was filtered: its raw value must not count
    let linear = rank(&spec(&[score(Curve::Linear, 2.0)]), &items(4), &alive, &raw);
    let scores: Vec<f32> = linear.iter().map(|i| i.score).collect();
    assert_eq!(scores, vec![2.0, 1.0, 0.0]);
    assert_eq!(
        linear[0].position.x, 3.0,
        "the largest raw value ranks first"
    );
    let inverse = rank(
        &spec(&[score(Curve::Inverse, 1.0)]),
        &items(4),
        &alive,
        &raw,
    );
    assert_eq!(inverse[0].position.x, 0.0);
    let square = rank(&spec(&[score(Curve::Square, 1.0)]), &items(4), &alive, &raw);
    assert_eq!(square[1].score, 0.25);
    let sqrt = rank(&spec(&[score(Curve::Sqrt, 1.0)]), &items(4), &alive, &raw);
    assert!((sqrt[1].score - 0.5f32.sqrt()).abs() < 1e-6);
}

#[test]
fn equal_raw_values_give_each_item_the_full_weight() {
    let raw = vec![vec![7.0; 3], vec![f32::INFINITY; 3], vec![2.0; 3]];
    // Whatever the curve: an inverse one must not turn "all alike" into zero.
    let s = spec(&[
        score(Curve::Linear, 0.5),
        score(Curve::Square, 1.0),
        score(Curve::Inverse, 2.0),
    ]);
    let ranked = rank(&s, &items(3), &[0, 1, 2], &raw);
    assert!(ranked.iter().all(|i| i.score == 3.5), "{ranked:?}");
    let order: Vec<f32> = ranked.iter().map(|i| i.position.x).collect();
    assert_eq!(
        order,
        vec![0.0, 1.0, 2.0],
        "a tie goes to the earliest generated"
    );
}

fn ranked(scores: &[f32]) -> Vec<Item> {
    scores
        .iter()
        .enumerate()
        .map(|(i, &score)| Item {
            position: Vec3::X * i as f32,
            score,
        })
        .collect()
}

#[test]
fn best_draws_nothing_and_an_empty_query_draws_nothing() {
    let mut rng = Random::new(7);
    let before = rng.state();
    assert_eq!(
        pick(&ranked(&[3.0, 2.0]), RunMode::Best, &mut rng).map(|i| i.score),
        Some(3.0)
    );
    assert_eq!(pick(&[], RunMode::RandomBest25, &mut rng), None);
    assert_eq!(rng.state(), before, "no draw");
}

#[test]
fn a_random_mode_draws_once_from_the_top_of_the_score_range() {
    // Range 0..10: the top 25% is score ≥ 7.5, the top 5% score ≥ 9.5.
    let list = ranked(&[10.0, 9.6, 8.0, 7.5, 5.0, 0.0]);
    let mut seen = [false; 6];
    for seed in 0..200 {
        let mut rng = Random::new(seed);
        let mut once = Random::new(seed);
        once.value();
        let got = pick(&list, RunMode::RandomBest25, &mut rng).expect("an item");
        assert_eq!(rng.state(), once.state(), "exactly one draw");
        seen[got.position.x as usize] = true;
        let five = pick(&list, RunMode::RandomBest5, &mut Random::new(seed)).expect("an item");
        assert!(five.score >= 9.5, "{five:?}");
    }
    assert_eq!(seen, [true, true, true, true, false, false]);
}
