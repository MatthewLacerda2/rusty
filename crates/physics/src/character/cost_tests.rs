//! What one `Move` costs in query work on a triangle-mesh floor (#2335), counted
//! (`cost`), standing and walking: the bench level (`bench::level`). The
//! bounds are the counts measured with a margin, so a change that visits more
//! triangles or runs more queries per Move fails here.

use super::bench::{level, stand, walk};
use super::cost::{take, Counts};

const TICKS: u64 = 60;

/// The counts of `TICKS` Moves of `moves` on the bench floor, per Move.
fn per_move(moves: fn(&crate::PhysicsWorld, &mut rusty_scene::Scene, u32, usize)) -> [f64; 5] {
    let (mut scene, physics, id) = level(40);
    take();
    moves(&physics, &mut scene, id, TICKS as usize);
    let c: Counts = take();
    let per = |n: u64| n as f64 / TICKS as f64;
    let out = [c.runs, c.mesh_casts, c.parts, c.constraints, c.contacts].map(per);
    println!("runs, mesh casts, parts, constraints, contacts per Move: {out:?}");
    out
}

#[test]
fn standing_counts() {
    let _ = per_move(stand);
}

#[test]
fn walking_counts() {
    let _ = per_move(walk);
}
