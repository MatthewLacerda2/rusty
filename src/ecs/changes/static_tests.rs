//! src/ecs/changes/static_tests.rs — the `StaticSet` channel against the full pass it
//! lets a consumer skip (#1705): a set of the static entities active in the
//! hierarchy, kept from the cursor's ids alone, equals the set rebuilt from every
//! entity, after any sequence of writes. A writer that forgets to stamp breaks this.

use std::collections::BTreeSet;

use proptest::prelude::*;

use crate::ecs::{Changes, Cursor, StaticSet, World};

#[derive(Clone, Debug)]
enum Op {
    Spawn,
    SetStatic(usize, bool),
    SetActive(usize, bool),
    /// Re-parent under an older entity (so no cycle), children lists kept in step as
    /// `Scene::set_parent` keeps them; `None` unparents.
    Reparent(usize, Option<usize>),
    Despawn(usize),
    /// Overwrite in place with these `is_static` and `active` flags (prefab propagation).
    Overwrite(usize, bool, bool),
    Clear,
    Read,
}

fn op() -> impl Strategy<Value = Op> {
    let at = 0usize..10;
    prop_oneof![
        3 => Just(Op::Spawn),
        3 => (at.clone(), any::<bool>()).prop_map(|(i, s)| Op::SetStatic(i, s)),
        3 => (at.clone(), any::<bool>()).prop_map(|(i, a)| Op::SetActive(i, a)),
        3 => (at.clone(), proptest::option::of(at.clone())).prop_map(|(i, p)| Op::Reparent(i, p)),
        1 => at.clone().prop_map(Op::Despawn),
        1 => (at, any::<bool>(), any::<bool>()).prop_map(|(i, s, a)| Op::Overwrite(i, s, a)),
        1 => Just(Op::Clear),
        3 => Just(Op::Read),
    ]
}

fn pick(w: &World, i: usize) -> Option<u32> {
    w.ids().get(i).copied()
}

fn reparent(w: &mut World, child: u32, parent: Option<u32>) {
    if parent.is_some_and(|p| p >= child) {
        return;
    }
    if let Some(old) = w.parent_id(child) {
        w.remove_child(old, child);
    }
    w.set_parent_id(child, parent);
    if let Some(p) = parent {
        w.add_child(p, child);
    }
}

fn apply(w: &mut World, op: &Op) {
    let id = |w: &World, i: usize| pick(w, i).unwrap_or(u32::MAX);
    match *op {
        Op::Spawn => {
            w.spawn("e".into());
        }
        Op::SetStatic(i, s) => {
            w.set_static(id(w, i), s);
        }
        Op::SetActive(i, a) => {
            w.set_active(id(w, i), a);
        }
        Op::Reparent(i, p) => {
            if let Some(child) = pick(w, i) {
                let parent = p.and_then(|p| pick(w, p));
                reparent(w, child, parent);
            }
        }
        Op::Despawn(i) => {
            if let Some(dead) = pick(w, i) {
                w.despawn(dead);
            }
        }
        Op::Overwrite(i, s, a) => {
            if let Some(mut doc) = pick(w, i).and_then(|e| w.entity_document(e)) {
                (doc.is_static, doc.active) = (s, a);
                w.replace_entity(doc);
            }
        }
        Op::Clear => w.clear(),
        Op::Read => {}
    }
}

fn member(w: &World, id: u32) -> bool {
    w.is_static(id) && w.is_active_in_hierarchy(id)
}

/// The full pass: every static entity active in the hierarchy.
fn rebuilt(w: &World) -> BTreeSet<u32> {
    w.ids()
        .iter()
        .copied()
        .filter(|&id| member(w, id))
        .collect()
}

/// The incremental consumer: only the ids the cursor names are looked at.
fn catch_up(w: &mut World, cursor: &mut Cursor<StaticSet>, kept: &mut BTreeSet<u32>) {
    match w.changes(cursor) {
        Changes::Rebuild => *kept = rebuilt(w),
        Changes::Ids(ids) => {
            for id in ids {
                if member(w, id) {
                    kept.insert(id);
                } else {
                    kept.remove(&id);
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn the_kept_static_set_equals_the_full_rebuild(ops in proptest::collection::vec(op(), 1..60)) {
        let mut w = World::new();
        let mut cursor = w.change_cursor::<StaticSet>();
        let mut kept = BTreeSet::new();
        for op in ops.iter().chain([&Op::Read]) {
            let before = w.static_revision();
            apply(&mut w, op);
            if matches!(op, Op::Read) {
                catch_up(&mut w, &mut cursor, &mut kept);
                prop_assert_eq!(&kept, &rebuilt(&w));
            }
            prop_assert!(w.static_revision() >= before, "the revision never goes back");
        }
    }
}
