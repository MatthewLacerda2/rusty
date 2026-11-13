//! crates/physics/src/watch/tests.rs — the incremental sync is the full one (#1707): two
//! copies of a scene take the same random edits and steps, one synced from the
//! change log, one in full every step, and rapier holds the same bits in both
//! after every step, as do the scenes the bodies write back into.

use proptest::prelude::*;

use super::fixture_tests::{digest, fixture, scene_digest};
use super::ops_tests::{apply, op, Op, Phys, SETTLE_STEPS};
use crate::PhysicsWorld;
use rusty_components::RigidBodyComponent;
use rusty_ecs::{Changes, WorldPose};
use rusty_scene::Scene;

/// The fixture twice: `a` synced from the change log, `b` in full every step.
pub struct Twin {
    pub a: Scene,
    b: Scene,
    pub fast: PhysicsWorld,
    full: PhysicsWorld,
}

impl Twin {
    /// Both copies, built and never stepped.
    pub fn new() -> Self {
        let (a, b) = (fixture(), fixture());
        let (fast, mut full) = (PhysicsWorld::from_scene(&a), PhysicsWorld::from_scene(&b));
        full.tracked.always_full = true;
        Self { a, b, fast, full }
    }

    /// Apply `op` to both copies, which must agree after every step it takes.
    pub fn run(&mut self, op: &Op) -> Result<(), TestCaseError> {
        if matches!(op, Op::Settle) {
            for _ in 0..SETTLE_STEPS {
                self.run(&Op::Step)?;
            }
            return Ok(());
        }
        apply(&mut self.a, &mut self.fast, op);
        apply(&mut self.b, &mut self.full, op);
        if matches!(op, Op::Step | Op::Pause) {
            same("rapier", &digest(&self.fast), &digest(&self.full))?;
            same("scene", &scene_digest(&self.a), &scene_digest(&self.b))?;
        }
        Ok(())
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
        twin.run(op)
            .map_err(|e| TestCaseError::fail(format!("op {n}: {e}")))?;
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

/// The sequence that panicked rapier's solver (#1775): the ground is adopted by
/// an inactive parent that then gets a dynamic Rigidbody (a body built
/// disabled), a box is jointed to it, and the box turns static. The joint is
/// inert, and both syncs step on.
#[test]
fn a_joint_from_a_fixed_body_to_a_disabled_one_steps() {
    use rusty_ecs::changes::ops::{Edit, Op as W};
    agree(&[
        Op::World(W::On(0, Edit::Adopt(false))),
        Op::On(5, Phys::Body(false)),
        Op::Step,
        Op::On(1, Phys::Joint(Some(5))),
        Op::World(W::On(1, Edit::Overwrite(true, true))),
    ])
    .unwrap();
}

/// The sequence that panicked rapier's island manager (#1795): the world's first
/// step has `dt` 0 (Play starting paused), a dynamic body is despawned, and the
/// next real step steps on.
#[test]
fn a_paused_first_step_then_a_despawn_steps() {
    use rusty_ecs::changes::ops::{Edit, Op as W};
    agree(&[Op::Pause, Op::World(W::On(1, Edit::Despawn)), Op::Step]).unwrap();
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
