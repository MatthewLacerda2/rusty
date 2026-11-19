//! What one `CharacterController.Move` costs (#2335), timed: a capsule standing,
//! then walking, on a 60 m triangle-mesh floor of half-metre quads
//! (`physics::character_bench`). Here because the sim may not read a clock; the
//! counted gate run is `physics`' `character::cost_tests`.

use std::time::Instant;

use rusty_physics::character_bench::{level, stand, walk};
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

type Moves = fn(&PhysicsWorld, &mut Scene, u32, usize);

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn move_cost() {
    let ticks = 600;
    for (what, moves) in [("standing", stand as Moves), ("walking", walk)] {
        let (mut scene, physics, id) = level(120);
        let t = Instant::now();
        moves(&physics, &mut scene, id, ticks);
        let per = t.elapsed().as_secs_f64() * 1e6 / ticks as f64;
        println!("{what}: {per:.2} µs per Move");
    }
}
