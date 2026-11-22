//! Timed here because the sim may not read a clock; n = 5 and 20 rooms a side span a small
//! level to a Dust2-sized one.

use std::time::Instant;

use rusty_ai::perception::bench::{level_with_perceivers, run};
use rusty_ai::perception::test_world::PLAYER;
use rusty_ai::perception::SightRays;
use rusty_ai::query::bench::{level, query, query_far};

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
    let masks = [
        ("the target's layer", 1 << PLAYER),
        ("every layer", u32::MAX),
    ];
    for (looks_for, mask) in masks {
        let (mut scene, physics, bots) = level_with_perceivers(20, mask);
        let mut rays = SightRays::default();
        run(&mut scene, &physics, &mut rays, 10);
        let ticks = 600;
        let t = Instant::now();
        run(&mut scene, &physics, &mut rays, ticks);
        let per = t.elapsed().as_secs_f64() * 1e3 / ticks as f64;
        let n = bots.len();
        println!("{n} perceivers with bodies looking for {looks_for}: {per:.4} ms per tick");
    }
}
