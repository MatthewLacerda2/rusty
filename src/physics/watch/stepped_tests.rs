//! src/physics/watch/stepped_tests.rs — a body the step changed that no change
//! named (#1842): rapier's solver rewrites the pose of every body in its active
//! set, a body just turned static included, and no write-back stamps a static
//! body, so the incremental sync must visit it anyway.

use super::ops_tests::{Op, Phys};
use super::tests::{agree, Twin};

/// The sequence that drifted (#1842): an inactive parent adopts entity 1, a
/// settled box (entity 3) turns static, two steps, then a paused one. The full
/// pass re-pushed the box's rotation on the paused step, the incremental one left
/// the solver's.
fn ops() -> Vec<Op> {
    use crate::ecs::changes::ops_tests::{Edit, Op as W};
    vec![
        Op::Settle,
        Op::On(2, Phys::Velocity(-22)),
        Op::World(W::On(1, Edit::Adopt(false))),
        Op::Step,
        Op::Step,
        Op::Settle,
        Op::World(W::On(2, Edit::SetStatic(true))),
        Op::Step,
        Op::Step,
        Op::Pause,
    ]
}

#[test]
fn a_body_turned_static_steps_alike() {
    agree(&ops()).unwrap();
}

/// The case's precondition: the step after the box turns static hands back a
/// rotation that is not the one its Transform pushed. Without it the test above
/// would pass whatever the sync visits.
#[test]
fn the_solver_moves_a_body_just_turned_static() {
    let ops = ops();
    let mut twin = Twin::new();
    for op in &ops[..8] {
        twin.run(op).unwrap();
    }
    let id = 3;
    let held = twin.a.world.transform(id).unwrap().rotation;
    let body = &twin.fast.bodies[twin.fast.id_to_body[&id]];
    assert!(body.is_fixed(), "entity {id} is the box turned static");
    assert_ne!(body.position().rotation, held);
}
