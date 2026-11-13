//! crates/ecs/src/changes/derived_tests.rs — every derived channel, and the Transform column,
//! against the full pass it lets a consumer skip (#1705): a value kept per id from a
//! cursor's ids alone equals the value rebuilt from every entity, after any sequence
//! of writes (`ops.rs`). Two consumers read each channel, at different paces.
//! A writer that forgets to stamp, or stamps an id without its subtree, breaks it.

use std::collections::BTreeMap;
use std::fmt::Debug;

use proptest::prelude::*;

use super::ops::{apply, Edit, Op};
use crate::{Activity, Changes, Channel, Cursor, Layer, StaticSet, World, WorldPose};
use rusty_components::TransformComponent;

type Kept<V> = BTreeMap<u32, V>;

fn rebuilt<V>(w: &World, value: &impl Fn(&World, u32) -> Option<V>) -> Kept<V> {
    w.ids()
        .iter()
        .filter_map(|&id| Some((id, value(w, id)?)))
        .collect()
}

fn catch_up<T: Channel, V>(
    w: &mut World,
    cursor: &mut Cursor<T>,
    kept: &mut Kept<V>,
    value: &impl Fn(&World, u32) -> Option<V>,
) {
    match w.changes(cursor) {
        Changes::Rebuild => *kept = rebuilt(w, value),
        Changes::Ids(ids) => {
            for id in ids {
                match value(w, id).filter(|_| w.contains(id)) {
                    Some(v) => kept.insert(id, v),
                    None => kept.remove(&id),
                };
            }
        }
    }
}

/// Run `ops` with two cursors on `T` (one reading at every `Read`, one at every
/// other), each keeping `value` per id from its ids alone, checked at each read.
fn kept_equals_rebuilt<T: Channel, V: PartialEq + Debug>(
    ops: &[Op],
    value: impl Fn(&World, u32) -> Option<V>,
) -> Result<(), TestCaseError> {
    let mut w = World::new();
    let (mut fast, mut slow) = (w.change_cursor::<T>(), w.change_cursor::<T>());
    let (mut kept_fast, mut kept_slow) = (Kept::new(), Kept::new());
    let mut reads = 0;
    for op in ops.iter().chain([&Op::Read, &Op::Read]) {
        let before = w.static_revision();
        apply(&mut w, op);
        prop_assert!(
            w.static_revision() >= before,
            "the revision never goes back"
        );
        if !matches!(op, Op::Read) {
            continue;
        }
        reads += 1;
        catch_up(&mut w, &mut fast, &mut kept_fast, &value);
        prop_assert_eq!(&kept_fast, &rebuilt(&w, &value));
        if reads % 2 == 0 {
            catch_up(&mut w, &mut slow, &mut kept_slow, &value);
            prop_assert_eq!(&kept_slow, &rebuilt(&w, &value));
        }
    }
    Ok(())
}

fn bits(m: glam::Mat4) -> [u32; 16] {
    m.to_cols_array().map(f32::to_bits)
}

proptest! {
    #[test]
    fn the_static_set(ops in proptest::collection::vec(crate::edit_ops!(), 1..60)) {
        let member = |w: &World, id| (w.is_static(id) && w.is_active_in_hierarchy(id)).then_some(());
        kept_equals_rebuilt::<StaticSet, _>(&ops, member)?;
    }

    #[test]
    fn active_in_hierarchy(ops in proptest::collection::vec(crate::edit_ops!(), 1..60)) {
        kept_equals_rebuilt::<Activity, _>(&ops, |w, id| Some(w.is_active_in_hierarchy(id)))?;
    }

    #[test]
    fn layers(ops in proptest::collection::vec(crate::edit_ops!(), 1..60)) {
        kept_equals_rebuilt::<Layer, _>(&ops, |w, id| Some(w.layer(id)))?;
    }

    #[test]
    fn world_poses(ops in proptest::collection::vec(crate::edit_ops!(), 1..60)) {
        kept_equals_rebuilt::<WorldPose, _>(&ops, |w, id| Some(bits(w.local_to_world(id))))?;
    }

    #[test]
    fn local_transforms(ops in proptest::collection::vec(crate::edit_ops!(), 1..60)) {
        let local = |w: &World, id| w.transform(id).map(|t| bits(t.to_matrix()));
        kept_equals_rebuilt::<TransformComponent, _>(&ops, local)?;
    }
}

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
