//! src/physics/watch/ops_tests.rs — the ops the incremental sync's equivalence
//! proptest runs (#1707, `tests.rs`): the World's own shared edit sequence
//! (`ecs::changes::ops_tests`), the physics edits (`edit_tests.rs`), a collision
//! matrix change and steps, `Time.timeScale` 0 among them.

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
}

pub fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => ops_tests::op().prop_map(Op::World),
        8 => (0usize..10, phys()).prop_map(|(i, p)| Op::On(i, p)),
        1 => (0u8..3, any::<bool>()).prop_map(|(l, on)| Op::Matrix(l, on)),
        6 => Just(Op::Step),
        1 => Just(Op::Pause),
    ]
}

/// Apply `op` to `scene`; a step steps `physics`.
pub fn apply(scene: &mut Scene, physics: &mut PhysicsWorld, op: &Op) {
    let (e, edit) = match op {
        Op::World(op) => return world_op(&mut scene.world, op),
        Op::Matrix(l, on) => return scene.collision_matrix.set_collision(0, *l, *on),
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
