//! src/physics/watch/tests.rs — the incremental sync is the full one (#1707): two
//! copies of a scene take the same random edits and steps, one synced from the
//! change log, one in full every step, and rapier holds the same bits in both
//! after every step, as do the scenes the bodies write back into.

use std::panic::{catch_unwind, AssertUnwindSafe};

use proptest::prelude::*;

use super::fixture_tests::{digest, fixture, scene_digest};
use super::ops_tests::{apply, op, Op, SETTLE_STEPS};
use crate::components::RigidBodyComponent;
use crate::ecs::{Changes, WorldPose};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// The fixture twice: `a` synced from the change log, `b` in full every step.
pub struct Twin {
    pub a: Scene,
    b: Scene,
    pub fast: PhysicsWorld,
    full: PhysicsWorld,
}

impl Twin {
    pub fn new() -> Self {
        let (a, b) = (fixture(), fixture());
        let (fast, mut full) = (PhysicsWorld::from_scene(&a), PhysicsWorld::from_scene(&b));
        full.tracked.always_full = true;
        Self { a, b, fast, full }
    }

    /// Apply `op` to both copies, which must agree after every step it takes;
    /// `Ok(false)` when rapier panicked in both, which ends the run.
    pub fn run(&mut self, op: &Op) -> Result<bool, TestCaseError> {
        if matches!(op, Op::Settle) {
            for _ in 0..SETTLE_STEPS {
                if !self.run(&Op::Step)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        // A joint between two bodies rapier cannot solve panics inside its step,
        // on both syncs alike (#1775): that ends the run, and must end both.
        let ran_a = catch_unwind(AssertUnwindSafe(|| apply(&mut self.a, &mut self.fast, op)));
        let ran_b = catch_unwind(AssertUnwindSafe(|| apply(&mut self.b, &mut self.full, op)));
        prop_assert_eq!(
            ran_a.is_ok(),
            ran_b.is_ok(),
            "only one panicked at {:?}",
            op
        );
        if ran_a.is_err() {
            return Ok(false);
        }
        if matches!(op, Op::Step | Op::Pause) {
            same("rapier", &digest(&self.fast), &digest(&self.full))?;
            same("scene", &scene_digest(&self.a), &scene_digest(&self.b))?;
        }
        Ok(true)
    }
}

/// `l` and `r` are the same text; otherwise fail with the lines that differ.
fn same(what: &str, l: &str, r: &str) -> Result<(), TestCaseError> {
    let diff: Vec<_> = l.lines().zip(r.lines()).filter(|(a, b)| a != b).collect();
    prop_assert!(l == r, "{} differs: {:#?}", what, diff);
    Ok(())
}

/// Run `ops`, then two steps, on a [`Twin`].
pub fn agree(ops: &[Op]) -> Result<(), TestCaseError> {
    let mut twin = Twin::new();
    for (n, op) in ops.iter().chain(&[Op::Step, Op::Step]).enumerate() {
        let ran = twin
            .run(op)
            .map_err(|e| TestCaseError::fail(format!("op {n}: {e}")))?;
        if !ran {
            return Ok(());
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
