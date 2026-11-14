//! The time-sliced query (#2116): served at any budget, it answers exactly what
//! the synchronous [`run`] answers, a random mode's one draw included; queries
//! wait their turn oldest first; and a ticket reads as gone once read, cancelled,
//! cleared, or its querier dropped.

use glam::Vec3;

use super::queue::Status;
use super::test_world::{wall, Arena};
use super::{pick, run, Curve, Filter, Generator, Measure, Progress, QueryQueue, QuerySpec};
use super::{RunMode, Score, Test};
use rusty_core::random::Random;

/// Walls with gaps between the querier and a 24-item donut, so the search is
/// long; `exists` then `length` (two searches), and a distance score.
fn level() -> (Arena, u32, QuerySpec) {
    let mut arena = Arena::new(|s| {
        wall(s, (-2.0, -15.0), (-1.0, 12.0));
        wall(s, (4.0, -12.0), (5.0, 15.0));
    });
    let bot = arena.agent(Vec3::new(-12.0, 0.0, 0.0));
    let test = |measure, filter, score| Test {
        measure,
        filter,
        score,
    };
    let score = Some(Score {
        curve: Curve::Inverse,
        weight: 1.0,
    });
    let spec = QuerySpec {
        generator: Generator::Donut {
            center: Vec3::new(10.0, 0.0, 0.0),
            inner: 2.0,
            outer: 8.0,
            rings: 2,
            per_ring: 12,
        },
        project: 2.0,
        tests: vec![
            test(Measure::PathLength, None, score),
            test(Measure::PathExists, Some(Filter::Wanted), None),
            test(Measure::Separation { radius: 30.0 }, None, score),
        ],
    };
    (arena, bot, spec)
}

/// Request `spec` and serve the queue at `budget` until it answers: the answer,
/// and how many ticks it took.
fn sliced(
    arena: &Arena,
    bot: u32,
    spec: &QuerySpec,
    mode: RunMode,
    budget: u32,
) -> (Option<super::Item>, u32) {
    let (mut queue, mut rng) = (QueryQueue::default(), Random::new(7));
    queue.set_iterations_per_frame(budget);
    let progress = Progress::start(&arena.world(), arena.anchor(bot), spec.clone());
    let ticket = queue.request(bot, progress, mode, &mut rng);
    let mut ticks = 0;
    loop {
        ticks += 1;
        queue.serve(&arena.nav, &mut rng);
        if let Status::Done(item) = queue.poll(ticket) {
            return (item, ticks);
        }
        assert!(ticks < 100_000, "it finishes");
    }
}

#[test]
fn a_sliced_query_answers_what_run_answers_at_any_budget() {
    let (arena, bot, spec) = level();
    let ranked = run(&arena.world(), arena.anchor(bot), &spec);
    assert!(ranked.len() > 4, "most of the donut is reachable");
    for mode in [RunMode::Best, RunMode::RandomBest25] {
        let want = pick(&ranked, mode, &mut Random::new(7));
        for budget in [1, 50, u32::MAX] {
            let (got, ticks) = sliced(&arena, bot, &spec, mode, budget);
            assert_eq!(got, want, "{mode:?} at {budget} a tick");
            assert_eq!(ticks == 1, budget == u32::MAX, "{budget}: {ticks} ticks");
        }
    }
}

#[test]
fn queries_wait_oldest_first_and_one_without_a_search_answers_at_once() {
    let (arena, bot, spec) = level();
    let (mut queue, mut rng) = (QueryQueue::default(), Random::new(1));
    queue.set_iterations_per_frame(200);
    let start = || Progress::start(&arena.world(), arena.anchor(bot), spec.clone());
    let (a, b) = (
        queue.request(bot, start(), RunMode::Best, &mut rng),
        queue.request(bot, start(), RunMode::Best, &mut rng),
    );
    let mut cheap = spec.clone();
    cheap.tests.truncate(0);
    let c = queue.request(
        bot,
        Progress::start(&arena.world(), arena.anchor(bot), cheap),
        RunMode::Best,
        &mut rng,
    );
    assert!(
        matches!(queue.poll(c), Status::Done(Some(_))),
        "no search to wait for"
    );
    let mut done = Vec::new();
    while done.len() < 2 {
        queue.serve(&arena.nav, &mut rng);
        for t in [a, b] {
            if !done.contains(&t) && queue.poll(t) != Status::Pending {
                done.push(t);
            }
        }
    }
    assert_eq!(done, [a, b], "the older answers first");
}

#[test]
fn a_ticket_is_gone_once_read_cancelled_cleared_or_its_querier_dropped() {
    let (arena, bot, spec) = level();
    let (mut queue, mut rng) = (QueryQueue::default(), Random::new(1));
    let mut ask = |q: &mut QueryQueue| {
        let progress = Progress::start(&arena.world(), arena.anchor(bot), spec.clone());
        q.request(bot, progress, RunMode::Best, &mut rng)
    };
    let read = ask(&mut queue);
    queue.set_iterations_per_frame(u32::MAX);
    queue.serve(&arena.nav, &mut Random::new(1));
    assert!(matches!(queue.poll(read), Status::Done(Some(_))));
    assert_eq!(queue.poll(read), Status::Gone, "read once");
    let cancelled = ask(&mut queue);
    assert!(queue.cancel(cancelled) && !queue.cancel(cancelled));
    assert_eq!(queue.poll(cancelled), Status::Gone);
    let dropped = ask(&mut queue);
    queue.retain_owners(|id| id != bot);
    assert_eq!(queue.poll(dropped), Status::Gone, "its querier went");
    let kept = ask(&mut queue);
    queue.clear();
    assert_eq!(queue.poll(kept), Status::Gone, "a scene load");
    assert!(
        ask(&mut queue) > kept,
        "a ticket is never reused within a run"
    );
    assert_eq!(queue.poll(0), Status::Gone);
    assert_eq!(queue.pending(), 1);
}
