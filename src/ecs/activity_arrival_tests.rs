//! src/ecs/activity_arrival_tests.rs — a parent that arrives after the child naming
//! it (#1805). A scene or prefab file can name a parent id it does not carry; when
//! that id later arrives (a spawn reaching it, a document put back), the kept
//! `activeInHierarchy` answers below it must follow the new parent's flag.

use crate::components::Entity;
use crate::ecs::World;

/// A document naming `parent`, as a load or a prefab stamp inserts it.
fn orphan(id: u32, parent: u32) -> Entity {
    let mut e = Entity::new(id, "orphan".into());
    e.parent_id = Some(parent);
    e
}

/// The proptest's minimal case, as its `cc` line in
/// `proptest-regressions/ecs/activity_tests.txt` records it.
#[test]
fn an_overwrite_naming_a_future_id_follows_that_id_once_it_spawns() {
    let mut w = World::new();
    for _ in 0..6 {
        w.spawn("e".into());
    }
    let mut doc = w.entity_document(1).expect("1 is live");
    (doc.active, doc.parent_id) = (true, Some(12));
    w.overwrite(doc);
    w.spawn("e".into());
    let mut late = Entity::new(11, "late".into());
    late.active = false;
    w.insert_entity(late);
    assert!(w.is_active_in_hierarchy(1), "12 is not live: 1 is a root");
    assert_eq!(w.spawn("e".into()), 12);
    w.set_active(12, false);
    assert!(!w.is_active_in_hierarchy(1), "12 arrived and went dark");
}

#[test]
fn a_loaded_orphan_goes_dark_when_its_parent_spawns_and_turns_off() {
    let mut w = World::new();
    w.insert_entity(orphan(1, 4));
    assert!(w.is_active_in_hierarchy(1));
    let parent = (0..3).map(|_| w.spawn("e".into())).last();
    assert_eq!(parent, Some(4));
    assert!(w.is_active_in_hierarchy(1));
    w.set_active(4, false);
    assert!(!w.is_active_in_hierarchy(1));
    w.set_active(4, true);
    assert!(w.is_active_in_hierarchy(1));
}

#[test]
fn a_parent_put_back_listing_its_child_keeps_the_subtree_walks() {
    // Undo puts a step's entities back in id order: a child can arrive before the
    // parent whose document lists it. That parent is no one-sided link.
    let mut w = World::new();
    w.insert_entity(orphan(1, 2));
    let mut parent = Entity::new(2, "parent".into());
    parent.children = vec![1];
    w.restore_entity(parent, 0);
    assert!(
        !w.activity.is_unlisted(),
        "both sides of the link are written"
    );
    assert!(w.is_active_in_hierarchy(1));
    w.set_active(2, false);
    assert!(!w.is_active_in_hierarchy(1));
}

#[test]
fn a_waiting_child_that_left_is_not_counted_against_the_arrival() {
    let mut w = World::new();
    w.insert_entity(orphan(1, 3));
    w.set_parent_id(1, None);
    w.spawn("e".into());
    assert_eq!(w.spawn("e".into()), 3);
    assert!(!w.activity.is_unlisted(), "1 no longer names 3");
}
