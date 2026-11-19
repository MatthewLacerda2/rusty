//! crates/ecs/src/changes/derived_case_tests.rs — the cases `derived_tests`' op
//! sequences once found, each held as its fixed sequence.

use super::derived_tests::{bits, kept_equals_rebuilt};
use super::ops::{apply, Edit, Op};
use crate::{Activity, World, WorldPose};

/// The flake of #1777: an Undo of a despawn put the child back without its place in
/// the parent's `children`, so switching the parent off missed it.
#[test]
fn undoing_a_despawn_keeps_the_child_in_its_parents_subtree() {
    use Edit::*;
    let ops = [
        Op::Spawn,
        Op::Spawn,
        Op::On(0, Adopt(true)),
        Op::On(1, Despawn),
        Op::On(0, Adopt(false)),
        Op::On(0, UndoDespawn),
        Op::Read,
        Op::On(2, SetActive(true)),
        Op::Read,
    ];
    kept_equals_rebuilt::<Activity, _>(&ops, |w, id| Some(w.is_active_in_hierarchy(id)))
        .expect("kept equals rebuilt");
}

/// #1853: an adopted entity's parent is younger than it, so "re-parent under an
/// older entity" made a cycle `Scene::set_parent` refuses, and a cycle has no world
/// pose to keep. The op generator now refuses one too.
#[test]
fn a_reparent_under_an_adopted_child_is_refused() {
    use Edit::*;
    let ops = [
        [Op::Spawn, Op::Spawn, Op::Spawn, Op::Spawn, Op::Spawn].as_slice(),
        &[Op::On(0, Adopt(false)), Op::On(0, Move(1)), Op::Spawn],
        &[Op::On(5, Reparent(Some(0))), Op::Read],
        &[Op::On(1, Overwrite(false, false)), Op::Spawn, Op::Read],
    ]
    .concat();
    let mut w = World::new();
    ops[..8].iter().for_each(|op| apply(&mut w, op));
    let (e, before) = (w.ids()[5], w.parent_id(w.ids()[5]));
    apply(&mut w, &ops[8]);
    assert_eq!(w.parent_id(e), before, "a cycle is refused");
    let poses = |w: &World, id| Some(bits(w.local_to_world(id)));
    kept_equals_rebuilt::<WorldPose, _>(&ops, poses).expect("kept equals rebuilt");
}
