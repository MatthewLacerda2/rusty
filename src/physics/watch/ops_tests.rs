//! src/physics/watch/ops_tests.rs — the ops the incremental sync's equivalence
//! proptest runs (#1707, `tests.rs`): the World's own shared edit sequence
//! (`ecs::changes::ops_tests`), the physics edits (`edit_tests.rs`), a collision
//! matrix change and steps: `Time.timeScale` 0 among them, and a run long
//! enough for bodies to fall asleep.

use proptest::prelude::*;

pub use super::edit_tests::Phys;
use super::edit_tests::{edit_one, phys};
use super::fixture_tests::world_op;
use crate::ecs::changes::ops_tests;
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

#[derive(Clone, Debug)]
pub enum Op {
    World(ops_tests::Op),
    On(usize, Phys),
    Matrix(u8, bool),
    Step,
    /// A step at `Time.timeScale` 0: `dt` is 0.
    Pause,
    /// [`SETTLE_STEPS`] steps: long enough for the fixture's bodies to land and
    /// fall asleep, so later edits and hits find sleeping bodies.
    Settle,
}

/// The fixture's highest body lands in about 50 steps; rapier puts a body still
/// for 0.5 s (30 steps) to sleep.
pub const SETTLE_STEPS: usize = 90;

pub fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => ops_tests::op().prop_map(Op::World),
        8 => (0usize..10, phys()).prop_map(|(i, p)| Op::On(i, p)),
        1 => (0u8..3, any::<bool>()).prop_map(|(l, on)| Op::Matrix(l, on)),
        6 => Just(Op::Step),
        1 => Just(Op::Pause),
        1 => Just(Op::Settle),
    ]
}

/// Apply `op` to `scene`; a step steps `physics`.
pub fn apply(scene: &mut Scene, physics: &mut PhysicsWorld, op: &Op) {
    let (e, edit) = match op {
        Op::World(op) => return world_op(&mut scene.world, op),
        Op::Matrix(l, on) => return scene.collision_matrix.set_collision(0, *l, *on),
        Op::Settle => {
            for _ in 0..SETTLE_STEPS {
                physics.step(scene, 1.0 / 60.0);
            }
            return;
        }
        Op::Step | Op::Pause => {
            let dt = if matches!(op, Op::Step) {
                1.0 / 60.0
            } else {
                0.0
            };
            physics.step(scene, dt);
            return;
        }
        Op::On(i, edit) => match scene.world.ids().get(*i).copied() {
            Some(e) => (e, edit),
            None => return,
        },
    };
    edit_one(&mut scene.world, e, edit);
}
