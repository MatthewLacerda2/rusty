//! src/ecs/changes/ops_tests.rs — the random edit sequences every channel's proptest
//! runs (#1705, `derived_tests.rs`). Each op is a write path a consumer can meet,
//! `insert_entity` and both branches of `restore_entity` among them.

use proptest::prelude::*;

use crate::components::Entity;
use crate::ecs::World;

#[derive(Clone, Debug)]
pub enum Op {
    Spawn,
    Clear,
    Read,
    /// An edit of the entity at this index of the insertion order (none past its end).
    On(usize, Edit),
}

#[derive(Clone, Debug)]
pub enum Edit {
    SetStatic(bool),
    SetActive(bool),
    SetLayer(u8),
    /// A Transform write; the small range lands the value already held too.
    Move(u8),
    /// Re-parent under an older entity (a younger one unparents, so no cycle),
    /// children lists kept in step as `Scene::set_parent` keeps them.
    Reparent(Option<usize>),
    Despawn,
    /// Overwrite in place with these `is_static` and `active` flags (prefab propagation).
    Overwrite(bool, bool),
    /// A load's `insert_entity` of a new parent (`active` given) over an orphan that
    /// already names it, as a file listing the child first does.
    Adopt(bool),
    /// An Undo of a flipped `active` flag (`restore_entity` of a live id) …
    UndoFlip,
    /// … and of a despawn (`restore_entity` of a dead one, and of the live parent
    /// and children the despawn unlinked, which the journal recorded too).
    UndoDespawn,
}

fn edit() -> impl Strategy<Value = Edit> {
    let flag = any::<bool>;
    prop_oneof![
        3 => flag().prop_map(Edit::SetStatic),
        3 => flag().prop_map(Edit::SetActive),
        2 => (0u8..3).prop_map(Edit::SetLayer),
        3 => (0u8..3).prop_map(Edit::Move),
        3 => proptest::option::of(0usize..10).prop_map(Edit::Reparent),
        1 => Just(Edit::Despawn),
        1 => (flag(), flag()).prop_map(|(s, a)| Edit::Overwrite(s, a)),
        1 => flag().prop_map(Edit::Adopt),
        1 => Just(Edit::UndoFlip),
        1 => Just(Edit::UndoDespawn),
    ]
}

pub fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => Just(Op::Spawn),
        1 => Just(Op::Clear),
        3 => Just(Op::Read),
        19 => (0usize..10, edit()).prop_map(|(i, e)| Op::On(i, e)),
    ]
}

fn reparent(w: &mut World, child: u32, parent: Option<u32>) {
    if let Some(old) = w.parent_id(child) {
        w.remove_child(old, child);
    }
    if let Some(p) = parent {
        w.add_child(p, child); // first, as `Scene::set_parent` does
    }
    w.set_parent_id(child, parent);
}

fn adopt(w: &mut World, child: u32, active: bool) {
    let parent = w.next_id();
    reparent(w, child, None);
    w.set_parent_id(child, Some(parent)); // the orphan: its parent is not live yet
    let mut doc = Entity::new(parent, "parent".into());
    (doc.active, doc.children) = (active, vec![child]);
    w.insert_entity(doc);
}

pub fn apply(w: &mut World, op: &Op) {
    let pick = |w: &World, i: usize| w.ids().get(i).copied();
    let (e, edit) = match op {
        Op::Spawn => {
            w.spawn("e".into());
            return;
        }
        Op::Clear => return w.clear(),
        Op::Read => return,
        Op::On(i, edit) => match pick(w, *i) {
            Some(e) => (e, edit),
            None => return,
        },
    };
    match *edit {
        Edit::SetStatic(s) => {
            w.set_static(e, s);
        }
        Edit::SetActive(a) => {
            w.set_active(e, a);
        }
        Edit::SetLayer(l) => {
            w.set_layer(e, l);
        }
        Edit::Move(x) => w.transform_mut(e).expect("live").position.x = f32::from(x),
        Edit::Reparent(p) => {
            let parent = p.and_then(|p| pick(w, p)).filter(|&p| p < e);
            reparent(w, e, parent);
        }
        Edit::Despawn => w.despawn(e),
        Edit::Overwrite(s, a) => {
            let mut doc = w.entity_document(e).expect("live");
            (doc.is_static, doc.active) = (s, a);
            w.replace_entity(doc);
        }
        Edit::Adopt(a) => adopt(w, e, a),
        Edit::UndoFlip => undo(w, |w| {
            w.set_active(e, !w.is_active_self(e));
        }),
        Edit::UndoDespawn => undo(w, |w| w.despawn(e)),
    }
}

/// Run `edit` under the journal and put back every entity it recorded, as
/// `scene::undo` does: a despawn's parent and children come back with it (#1777).
fn undo(w: &mut World, edit: impl FnOnce(&mut World)) {
    w.journal_resume();
    edit(w);
    w.journal_pause();
    let taken = w.journal_take().expect("the edit wrote");
    for recorded in taken.before.into_values().flatten() {
        w.restore_entity(recorded.doc, recorded.seq);
    }
}
