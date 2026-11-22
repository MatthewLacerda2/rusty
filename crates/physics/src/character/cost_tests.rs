use super::bench::{level, stand, walk};
use super::cost::take;
use crate::PhysicsWorld;
use rusty_scene::Scene;

const TICKS: usize = 60;

const WALKING: [f64; 5] = [1.0, 3.0, 34.0, 2.0, 2.0];

type Moves = fn(&PhysicsWorld, &mut Scene, u32, usize);

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

/// The least a Move on the mesh floor does: one rapier run, a mesh cast that
/// visits a triangle and hits it, so one constraint build and one `contact`.
/// A count under it means the floor stopped being a mesh or the counting
/// stopped, and every upper bound would pass on nothing.
const FLOOR: [f64; 5] = [1.0; 5];

fn within(counts: [f64; 5], bounds: [f64; 5]) {
    let names = ["runs", "mesh casts", "parts", "constraints", "contacts"];
    for (((n, got), min), max) in names.iter().zip(counts).zip(FLOOR).zip(bounds) {
        assert!(got >= min, "{n}: {got} per Move, under {min}: {counts:?}");
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
