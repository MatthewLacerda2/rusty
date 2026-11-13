//! src/physics/watch/sleep_tests.rs — the incremental sync across sleep (#1707):
//! a body at rest is the one the change log lets the sync and the write-back
//! skip, so these pin that it stays the full pass's body when an edit or a hit
//! wakes it, step by step over a long run.

use super::ops_tests::{Op, Phys};
use super::tests::{agree, Twin};
use crate::ecs::changes::ops::{Edit, Op as W};

/// The fixture's `i`th entity.
fn id(twin: &Twin, i: usize) -> u32 {
    twin.a.world.ids()[i]
}

/// Whether the fixture's `i`th entity's body is asleep in the change-log copy.
fn asleep(twin: &Twin, i: usize) -> bool {
    let p = &twin.fast;
    let body = p.id_to_body.get(&id(twin, i));
    body.is_some_and(|&h| p.bodies[h].is_sleeping())
}

/// Run `op` on `twin`; it must agree.
fn run(twin: &mut Twin, op: &Op) {
    let ran = twin.run(op);
    assert!(ran.is_ok(), "{op:?}: {ran:?}");
}

/// [`Op::Settle`] puts every fixture body to sleep: the proptest's long runs
/// reach sleeping bodies, not only falling ones.
#[test]
fn settling_puts_the_fixture_to_sleep() {
    let mut twin = Twin::new();
    run(&mut twin, &Op::Settle);
    for i in 1..4 {
        assert!(asleep(&twin, i), "body {i} still awake after settling");
    }
}

/// The committed replay proof: a settled scene, a body pushed into its sleeping
/// neighbour, which wakes and is pushed in turn, then 810 more steps while
/// everything settles again. Both copies hold the same bits after every step.
#[test]
fn a_sleeping_body_hit_by_a_moving_one_steps_as_the_full_pass() {
    let mut twin = Twin::new();
    run(&mut twin, &Op::Settle);
    assert!(asleep(&twin, 2), "the sphere must be asleep before the hit");
    let before = twin.a.world.transform(id(&twin, 2)).map(|t| t.position);
    run(&mut twin, &Op::On(1, Phys::Velocity(i8::MAX)));
    for _ in 0..9 {
        run(&mut twin, &Op::Settle);
    }
    let after = twin.a.world.transform(id(&twin, 2)).map(|t| t.position);
    assert_ne!(before, after, "the hit never reached the sleeping sphere");
}

/// A body disabled under a falling one keeps its world pose: the full write-back
/// re-reads it under its parent's new pose, so the narrowed one must reach it too,
/// though nothing named it before the step (the proptest's first find).
#[test]
fn a_disabled_body_under_a_falling_one_is_written_back() {
    let ops = vec![
        Op::World(W::On(2, Edit::Adopt(false))),
        Op::World(W::On(5, Edit::Reparent(Some(1)))),
        Op::Step,
        Op::Pause,
    ];
    let r = agree(&ops);
    assert!(r.is_ok(), "{r:?}");
}

/// A sleeping body whose parent is re-parented under a body that then moves: the
/// moved subtree is named, and the woken body steps as the full pass's does.
#[test]
fn a_settled_body_moved_by_a_reparent_wakes() {
    let ops = vec![
        Op::Settle,
        Op::World(W::On(3, Edit::Reparent(Some(1)))),
        Op::On(1, Phys::Velocity(-60)),
        Op::Settle,
    ];
    let r = agree(&ops);
    assert!(r.is_ok(), "{r:?}");
}
