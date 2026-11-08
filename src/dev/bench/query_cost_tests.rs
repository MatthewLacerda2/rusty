//! What one environment query costs (#1892), timed: the bench level's 64-point
//! donut with `trace` and `path` tests (`ai::query::query_cost_tests`), at the
//! Kolvik-sized `n = 5` and the Dust2-sized `n = 20`, on 1 m and 0.25 m navmesh
//! cells, then the `path` worst case (`query_far`: a bot 40 m off and one
//! survivor upstairs that no stair reaches). Here because the sim may not read
//! a clock. Beside it, what perception's step costs (#1900): 30 perceivers and
//! one target on the Dust2-sized level (`ai::perception::perception_cost_tests`).

use std::time::Instant;

use crate::ai::perception::perception_cost_tests::{level_with_perceivers, run};
use crate::ai::perception::SightRays;
use crate::ai::query::query_cost_tests::{level, query, query_far};

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn query_cost() {
    for (n, cell) in [(5, 1.0), (20, 1.0), (5, 0.25), (20, 0.25)] {
        let (scene, nav, physics) = level(n, cell);
        let runs = 50;
        let t = Instant::now();
        let mut found = 0;
        for _ in 0..runs {
            found = query(&scene, &nav, &physics);
        }
        let per = t.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
        let spans = nav.spans.len();
        println!("n={n} cell={cell} m: {per:.3} ms per query, {found} of 64 kept, {spans} spans");
        let t = Instant::now();
        for _ in 0..runs {
            found = query_far(&scene, &nav, &physics).len();
        }
        let per = t.elapsed().as_secs_f64() * 1e3 / f64::from(runs);
        println!("  bot 40 m off, one survivor upstairs: {per:.3} ms per query, {found} of 9 kept");
    }
}

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn perception_cost() {
    let (mut scene, physics, bots) = level_with_perceivers(20);
    let mut rays = SightRays::default();
    run(&mut scene, &physics, &mut rays, 10);
    let ticks = 600;
    let t = Instant::now();
    run(&mut scene, &physics, &mut rays, ticks);
    let per = t.elapsed().as_secs_f64() * 1e3 / ticks as f64;
    println!(
        "{} perceivers, one target: {per:.4} ms per tick",
        bots.len()
    );
}
