//! src/physics/watch_tests.rs — the incremental sync is the full one (#1707): two
//! copies of a scene take the same random edits and steps, one synced from the
//! change log, one in full every step, and rapier holds the same bits in both
//! after every step, as do the scenes the bodies write back into.

use std::panic::{catch_unwind, AssertUnwindSafe};

use proptest::prelude::*;

use super::watch_fixture_tests::{digest, fixture, scene_digest};
use super::watch_ops_tests::{apply, op, Op};
use super::PhysicsWorld;
use crate::components::RigidBodyComponent;
use crate::ecs::{Changes, WorldPose};

/// Run `ops` on two copies: incremental and full. Both must agree after each step.
fn agree(ops: &[Op]) -> Result<(), TestCaseError> {
    let (mut a, mut b) = (fixture(), fixture());
    let mut fast = PhysicsWorld::from_scene(&a);
    let mut full = PhysicsWorld::from_scene(&b);
    full.tracked.always_full = true;
    for (n, op) in ops.iter().chain(&[Op::Step, Op::Step]).enumerate() {
        // A joint between two bodies rapier cannot solve panics inside its step,
        // on both syncs alike (#1775): that ends the run, and must end both.
        let ran_a = catch_unwind(AssertUnwindSafe(|| apply(&mut a, &mut fast, op)));
        let ran_b = catch_unwind(AssertUnwindSafe(|| apply(&mut b, &mut full, op)));
        prop_assert_eq!(
            ran_a.is_ok(),
            ran_b.is_ok(),
            "only one panicked at op {}",
            n
        );
        if ran_a.is_err() {
            return Ok(());
        }
        if matches!(op, Op::Step | Op::Pause) {
            let (l, r) = (digest(&fast), digest(&full));
            let diff: Vec<_> = l.lines().zip(r.lines()).filter(|(a, b)| a != b).collect();
            prop_assert!(l == r, "rapier after op {}: {:#?}", n, diff);
            prop_assert_eq!(scene_digest(&a), scene_digest(&b), "scene after op {}", n);
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn incremental_sync_equals_the_full_pass(ops in proptest::collection::vec(op(), 1..60)) {
        agree(&ops)?;
    }
}

/// Bodies at rest write back what they hold, which stamps nothing: once the
/// fixture settles, a step names no entity on the channels the sync reads.
#[test]
fn a_settled_level_names_nothing() {
    let mut scene = fixture();
    let mut physics = PhysicsWorld::from_scene(&scene);
    let mut pose = scene.world.change_cursor::<WorldPose>();
    let mut rb = scene.world.change_cursor::<RigidBodyComponent>();
    for step in 0..3000 {
        physics.step(&mut scene, 1.0 / 60.0);
        let reads = [scene.world.changes(&mut pose), scene.world.changes(&mut rb)];
        let quiet = reads.iter().all(|c| *c == Changes::Ids(Vec::new()));
        if step > 60 && quiet {
            return;
        }
    }
    panic!("a body at rest still names itself every step");
}
