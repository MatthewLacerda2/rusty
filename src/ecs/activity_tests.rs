//! src/ecs/activity_tests.rs — the kept `activeInHierarchy` answers against the walk
//! they replaced (#1388): after any sequence of `Core` writes, every live id's answer
//! is `self_and_ancestors(id).all(is_active_self)`. A future writer that forgets to
//! invalidate breaks this. Parent links are mostly written with their `children`
//! entries, as `Scene::set_parent` writes them (the subtree invalidation walks
//! those, #1708), and sometimes raw, one-sided (the whole-store fallback).

use proptest::prelude::*;

use crate::components::Entity;
use crate::ecs::World;

/// One write. Ids are picked as an index into the live ids (or past them, for a dead,
/// fresh or orphan-named id), so a sequence stays meaningful as the world changes.
#[derive(Clone, Debug)]
enum Op {
    Spawn,
    SetActive(usize, bool),
    /// Parent `None` unparents; the parent may be the entity itself or a descendant.
    SetParent(usize, Option<usize>),
    /// The same link written raw: the parent's `children` does not list it.
    RawParent(usize, usize),
    Despawn(usize),
    /// An id no live entity has: a despawned one, or one an orphan may already name.
    Insert(u32, bool),
    Overwrite(usize, bool, Option<usize>),
    Clear,
    /// Ask one id, so later writes meet a partly warm cache.
    Read(usize),
    /// Ask every id, children before parents.
    ReadAll,
}

fn op() -> impl Strategy<Value = Op> {
    let at = 0usize..12;
    prop_oneof![
        3 => Just(Op::Spawn),
        3 => (at.clone(), any::<bool>()).prop_map(|(i, a)| Op::SetActive(i, a)),
        3 => (at.clone(), proptest::option::of(at.clone())).prop_map(|(i, p)| Op::SetParent(i, p)),
        1 => (at.clone(), at.clone()).prop_map(|(i, p)| Op::RawParent(i, p)),
        1 => at.clone().prop_map(Op::Despawn),
        1 => (1u32..16, any::<bool>()).prop_map(|(id, a)| Op::Insert(id, a)),
        1 => (at.clone(), any::<bool>(), proptest::option::of(at.clone()))
            .prop_map(|(i, a, p)| Op::Overwrite(i, a, p)),
        1 => Just(Op::Clear),
        3 => at.prop_map(Op::Read),
        2 => Just(Op::ReadAll),
    ]
}

/// The `i`th live id, wrapping; an id past them all when `i` overshoots, so a write
/// sometimes names a dead entity.
fn pick(w: &World, i: usize) -> u32 {
    let ids = w.ids();
    ids.get(i).copied().unwrap_or(i as u32 + 1)
}

fn apply(w: &mut World, op: &Op) {
    match *op {
        Op::Spawn => {
            w.spawn("e".into());
        }
        Op::SetActive(i, a) => {
            w.set_active(pick(w, i), a);
        }
        Op::SetParent(i, p) => {
            let parent = p.map(|p| pick(w, p));
            relink(w, pick(w, i), parent);
        }
        Op::RawParent(i, p) => {
            w.set_parent_id(pick(w, i), Some(pick(w, p)));
        }
        Op::Despawn(i) => {
            let id = pick(w, i);
            if w.contains(id) {
                w.despawn(id);
            }
        }
        Op::Insert(id, a) if !w.contains(id) => {
            let mut e = Entity::new(id, "late".into());
            e.active = a;
            w.insert_entity(e);
        }
        Op::Insert(..) => {}
        Op::Overwrite(i, a, p) => {
            let parent = p.map(|p| pick(w, p));
            if let Some(mut doc) = w.entity_document(pick(w, i)) {
                let id = doc.id;
                relink(w, id, None);
                (doc.active, doc.parent_id) = (a, parent);
                w.overwrite(doc);
                parent.map(|p| w.add_child(p, id));
            }
        }
        Op::Clear => w.clear(),
        Op::Read(_) | Op::ReadAll => {}
    }
}

/// Re-parent `id`, its old and new parents' `children` kept in step.
fn relink(w: &mut World, id: u32, parent: Option<u32>) {
    if let Some(old) = w.parent_id(id) {
        w.remove_child(old, id);
    }
    if w.contains(id) {
        parent.map(|p| w.add_child(p, id));
    }
    w.set_parent_id(id, parent);
}

fn walked(w: &World, id: u32) -> bool {
    w.contains(id) && w.self_and_ancestors(id).all(|c| w.is_active_self(c))
}

proptest! {
    #[test]
    fn every_kept_answer_equals_the_ancestor_walk(ops in proptest::collection::vec(op(), 1..60)) {
        let mut w = World::new();
        for op in ops.iter().chain([&Op::ReadAll]) {
            apply(&mut w, op);
            let asked: Vec<u32> = match *op {
                Op::Read(i) => vec![pick(&w, i)],
                Op::ReadAll => w.ids().iter().rev().copied().collect(),
                _ => continue,
            };
            for id in asked {
                prop_assert_eq!(w.is_active_in_hierarchy(id), walked(&w, id), "id {}", id);
            }
        }
    }
}
