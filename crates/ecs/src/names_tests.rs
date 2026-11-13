//! crates/ecs/src/names_tests.rs — the name index against the scan it replaced (#2089):
//! after any sequence of spawns, renames, despawns, re-parents, (de)activations,
//! overwrites, Undo steps and scene loads, `find_by_name` answers what walking
//! `ids()` for the first active entity with that name answers, for every name.

use proptest::prelude::*;

use crate::World;

const NAMES: [&str; 3] = ["a", "b", "c"];

#[derive(Clone, Debug)]
enum Op {
    Spawn(usize),
    Rename(usize, usize),
    Despawn(usize),
    Parent(usize, Option<usize>),
    SetActive(usize, bool),
    /// Overwrite in place with a renamed document (the prefab rebuild).
    Overwrite(usize, usize),
    /// Start recording an edit; `Undo` takes it back.
    Begin,
    Undo,
    /// Save and reload: clear, then insert every document.
    Load,
    /// No lookup before this op, so the next one catches up on several.
    Quiet,
}

fn op() -> impl Strategy<Value = Op> {
    let at = 0usize..10;
    let name = 0usize..NAMES.len();
    prop_oneof![
        4 => name.clone().prop_map(Op::Spawn),
        3 => (at.clone(), name.clone()).prop_map(|(i, n)| Op::Rename(i, n)),
        2 => at.clone().prop_map(Op::Despawn),
        2 => (at.clone(), proptest::option::of(at.clone())).prop_map(|(i, p)| Op::Parent(i, p)),
        3 => (at.clone(), any::<bool>()).prop_map(|(i, a)| Op::SetActive(i, a)),
        1 => (at.clone(), name).prop_map(|(i, n)| Op::Overwrite(i, n)),
        1 => Just(Op::Begin),
        1 => Just(Op::Undo),
        1 => Just(Op::Load),
        2 => Just(Op::Quiet),
    ]
}

/// The `i`th live id; an id past them all when `i` overshoots.
fn pick(w: &World, i: usize) -> u32 {
    w.ids().get(i).copied().unwrap_or(i as u32 + 1)
}

fn apply(w: &mut World, op: &Op) {
    match *op {
        Op::Spawn(n) => {
            w.spawn(NAMES[n].into());
        }
        Op::Rename(i, n) => {
            w.set_name(pick(w, i), NAMES[n].into());
        }
        Op::Despawn(i) => {
            let id = pick(w, i);
            if w.contains(id) {
                w.despawn(id);
            }
        }
        Op::Parent(i, p) => {
            let (id, parent) = (pick(w, i), p.map(|p| pick(w, p)));
            let cycle = parent.is_some_and(|p| w.self_and_ancestors(p).any(|a| a == id));
            if w.contains(id) && !cycle && parent.is_none_or(|p| w.contains(p)) {
                if let Some(old) = w.parent_id(id) {
                    w.remove_child(old, id);
                }
                parent.map(|p| w.add_child(p, id));
                w.set_parent_id(id, parent);
            }
        }
        Op::SetActive(i, a) => {
            w.set_active(pick(w, i), a);
        }
        Op::Overwrite(i, n) => {
            if let Some(mut doc) = w.entity_document(pick(w, i)) {
                doc.name = NAMES[n].into();
                w.replace_entity(doc);
            }
        }
        Op::Begin => w.journal_resume(),
        Op::Undo => undo(w),
        Op::Load => {
            let docs = w.collect_entities();
            w.clear();
            docs.into_iter().for_each(|d| w.insert_entity(d));
        }
        Op::Quiet => {}
    }
}

/// What `scene::undo` does with a recorded step: put every entity back as it was.
fn undo(w: &mut World) {
    w.journal_pause();
    for (id, before) in w.journal_take().map(|j| j.before).unwrap_or_default() {
        match before {
            Some(r) => w.restore_entity(r.doc, r.seq),
            None if w.contains(id) => w.despawn(id),
            None => {}
        }
    }
    w.rewind_changes();
}

/// The lookup the index replaced.
fn scanned(w: &World, name: &str) -> Option<u32> {
    w.ids()
        .iter()
        .copied()
        .find(|&id| w.name(id).is_some_and(|n| n.as_str() == name) && w.is_active_in_hierarchy(id))
}

proptest! {
    #[test]
    fn the_index_answers_what_the_scan_answers(ops in proptest::collection::vec(op(), 1..80)) {
        let mut w = World::new();
        for (k, op) in ops.iter().enumerate() {
            apply(&mut w, op);
            if matches!(ops.get(k + 1), Some(Op::Quiet)) {
                continue; // the next lookup catches up on this op and the ones after
            }
            for name in NAMES.iter().chain(&["missing"]) {
                prop_assert_eq!(w.find_by_name(name), scanned(&w, name), "{} after {:?}", name, op);
            }
        }
    }
}

#[test]
fn a_name_carried_by_an_inactive_first_entity_answers_the_next_active_one() {
    let mut w = World::new();
    let first = w.spawn("Enemy".into());
    let second = w.spawn("Enemy".into());
    assert_eq!(w.find_by_name("Enemy"), Some(first));
    w.set_active(first, false);
    assert_eq!(w.find_by_name("Enemy"), Some(second));
    w.set_name(second, "Boss".into());
    assert_eq!(w.find_by_name("Enemy"), None);
    assert_eq!(w.find_by_name("Boss"), Some(second));
}
