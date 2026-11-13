//! crates/physics/src/watch/borrow_tests.rs — the cases where the two syncs borrowed
//! different rapier bodies or colliders and so stepped apart (#1798): rapier marks
//! whatever is borrowed mutably as modified, and that order is the order the next
//! step meets new contact pairs in.

use super::ops_tests::{Op, Phys, SETTLE_STEPS};
use super::tests::{agree, Twin};

/// The sequence that drifted (#1798): a body's Rigidbody re-set, two spawns, a
/// collider on the second, a body disabled, then that collider re-parented under
/// the re-set body, which then settles. Its precondition holds too: some step
/// meets two new contact pairs at once, whose order the twins must agree on.
#[test]
fn a_collider_reparented_under_a_body_steps_alike() {
    use rusty_ecs::changes::ops::{Edit, Op as W};
    let s = Op::Step;
    let ops = [
        s.clone(),
        s.clone(),
        Op::On(1, Phys::Body(false)),
        Op::World(W::Spawn),
        s.clone(),
        Op::World(W::Spawn),
        Op::On(6, Phys::Collider(0)),
        s.clone(),
        Op::World(W::On(3, Edit::SetActive(false))),
        s.clone(),
        s.clone(),
        s.clone(),
        Op::World(W::On(6, Edit::Reparent(Some(1)))),
    ];
    let mut twin = Twin::new();
    let pairs = |t: &Twin| {
        t.fast
            .narrow_phase
            .contact_pairs()
            .filter(|c| c.has_any_active_contact())
            .count()
    };
    let mut most_new = 0;
    for op in ops.iter().chain(std::iter::repeat_n(&s, SETTLE_STEPS)) {
        let before = pairs(&twin);
        twin.run(op).unwrap();
        most_new = most_new.max(pairs(&twin).saturating_sub(before));
    }
    assert!(most_new >= 2, "no step met two new contact pairs at once");
}

/// A disabled kinematic body that moved keeps its old `position` (rapier copies
/// `next_position` only for an active body): neither sync may borrow it for that
/// on later steps, and re-enabled it steps alike again.
#[test]
fn a_disabled_kinematic_body_that_moved_steps_alike() {
    use rusty_ecs::changes::ops::{Edit, Op as W};
    agree(&[
        Op::Settle,
        Op::World(W::On(4, Edit::SetActive(false))),
        Op::World(W::On(4, Edit::Move(3))),
        Op::Step,
        Op::World(W::On(4, Edit::Move(7))),
        Op::Settle,
        Op::World(W::On(4, Edit::SetActive(true))),
        Op::Settle,
    ])
    .unwrap();
}
