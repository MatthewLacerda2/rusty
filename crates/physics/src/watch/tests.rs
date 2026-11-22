use proptest::prelude::*;

use super::fixture_tests::{digest, fixture, scene_digest};
use super::ops_tests::{apply, op, Op, Phys, SETTLE_STEPS};
use crate::PhysicsWorld;
use rusty_components::RigidBodyComponent;
use rusty_ecs::{Changes, WorldPose};
use rusty_scene::Scene;

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

fn same(what: &str, l: &str, r: &str) -> Result<(), TestCaseError> {
    let diff: Vec<_> = l.lines().zip(r.lines()).filter(|(a, b)| a != b).collect();
    prop_assert!(l == r, "{} differs: {:#?}", what, diff);
    Ok(())
}

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

#[test]
fn a_paused_first_step_then_a_despawn_steps() {
    use rusty_ecs::changes::ops::{Edit, Op as W};
    agree(&[Op::Pause, Op::World(W::On(1, Edit::Despawn)), Op::Step]).unwrap();
}

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

#[test]
fn edits_on_moving_bodies_sync_alike() {
    use rusty_ecs::changes::ops::{Edit, Op as W};
    agree(&[
        Op::Step,
        Op::On(4, Phys::NoBody),
        Op::World(W::On(4, Edit::Reparent(Some(1)))),
        Op::Step,
        Op::World(W::On(4, Edit::Reparent(Some(2)))),
        Op::Step,
        Op::On(3, Phys::ColliderActive(false)),
        Op::Step,
        Op::On(3, Phys::ColliderActive(true)),
        Op::Step,
        Op::World(W::On(2, Edit::SetLayer(1))),
        Op::On(2, Phys::Friction(2)),
        Op::Step,
        Op::On(1, Phys::Trigger(true)),
    ])
    .unwrap();
}
