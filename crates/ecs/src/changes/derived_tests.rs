use std::collections::BTreeMap;
use std::fmt::Debug;

use proptest::prelude::*;

use super::ops::{apply, Op};
use crate::{Activity, Changes, Channel, Children, Cursor, Layer, StaticSet, World, WorldPose};
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

pub(super) fn kept_equals_rebuilt<T: Channel, V: PartialEq + Debug>(
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

pub(super) fn bits(m: glam::Mat4) -> [u32; 16] {
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
    fn child_lists(ops in proptest::collection::vec(crate::edit_ops!(), 1..60)) {
        kept_equals_rebuilt::<Children, _>(&ops, |w, id| Some(w.children(id)))?;
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
