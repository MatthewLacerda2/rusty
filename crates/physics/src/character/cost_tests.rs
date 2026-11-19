//! What one `Move` costs in query work on a triangle-mesh floor (#2335), counted
//! (`cost`), standing and walking on the bench level (`bench::level`). The
//! bounds are the counts measured with a margin, so a change that visits more
//! triangles or runs more queries per Move fails here. Before the cut a Move
//! built every visited triangle's normal constraints and took a `contact` for
//! every hit that improved on the best so far; now each is taken about once
//! per mesh cast that hits.

use super::bench::{level, stand, walk};
use super::cost::take;
use crate::PhysicsWorld;
use rusty_scene::Scene;

const TICKS: usize = 60;

/// A walking Move's bounds: measured 1, 2.37, 27.35, 1.37, 1.37.
const WALKING: [f64; 5] = [1.0, 3.0, 34.0, 2.0, 2.0];

type Moves = fn(&PhysicsWorld, &mut Scene, u32, usize);

/// Per Move over `TICKS` Moves on a floor of `cells` × `cells` quads: rapier
/// runs, mesh casts, triangles visited, constraint builds, `contact` queries.
fn per_move(cells: u32, moves: Moves) -> [f64; 5] {
    let (mut scene, physics, id) = level(cells);
    take();
    moves(&physics, &mut scene, id, TICKS);
    let c = take();
    let grounded = scene.world.character_controller(id).map(|c| c.is_grounded);
    assert_eq!(
        grounded,
        Some(true),
        "the bench's capsule stays on its floor"
    );
    let per = |n: u64| n as f64 / TICKS as f64;
    [c.runs, c.mesh_casts, c.parts, c.constraints, c.contacts].map(per)
}

/// Each count no greater than its bound.
fn within(counts: [f64; 5], bounds: [f64; 5]) {
    let names = ["runs", "mesh casts", "parts", "constraints", "contacts"];
    for ((n, got), max) in names.iter().zip(counts).zip(bounds) {
        assert!(got <= max, "{n}: {got} per Move, over {max}: {counts:?}");
    }
}

#[test]
fn a_standing_move_builds_constraints_and_contacts_only_for_its_hits() {
    // Measured: 1, 2.5, 39.5, 1.5, 1.5 (39.5 constraints and 7.5 contacts before).
    within(per_move(40, stand), [1.0, 3.0, 48.0, 2.0, 2.0]);
}

#[test]
fn a_walking_move_builds_constraints_and_contacts_only_for_its_hits() {
    // 27.35 constraints and 3.48 contacts before.
    within(per_move(40, walk), WALKING);
}

#[test]
fn a_moves_work_does_not_grow_with_the_floor() {
    // A 16× larger floor: the BVH finds about the same few triangles (28.75).
    within(per_move(160, walk), WALKING);
}
