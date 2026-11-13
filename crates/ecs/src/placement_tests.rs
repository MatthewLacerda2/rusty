//! crates/ecs/src/placement_tests.rs — both sides of a placed parent link (#1805). A
//! document naming a parent its `children` lacks is listed there, at once when the
//! parent is live and when it arrives otherwise, so every subtree walk (activity,
//! world matrices, the change logs, a despawn's re-rooting) reaches the child.

use glam::Vec3;

use crate::{Activity, Changes, World};
use rusty_components::Entity;

/// A document naming `parent`, as a load or a prefab stamp inserts it.
fn orphan(id: u32, parent: u32) -> Entity {
    let mut e = Entity::new(id, "orphan".into());
    e.parent_id = Some(parent);
    e
}

/// A World holding orphan 1, waiting for 3, and then 3 spawned.
fn late_parent() -> World {
    let mut w = World::new();
    w.insert_entity(orphan(1, 3));
    w.spawn("e".into());
    assert_eq!(w.spawn("e".into()), 3);
    w
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
    w.overwrite(doc.clone());
    w.overwrite(doc);
    assert_eq!(w.awaited[&12].len(), 1, "one entry per dangling link");
    w.spawn("e".into());
    let mut late = Entity::new(11, "late".into());
    late.active = false;
    w.insert_entity(late);
    assert!(w.is_active_in_hierarchy(1), "12 is not live: 1 is a root");
    assert_eq!(w.spawn("e".into()), 12);
    assert_eq!(w.children(12), vec![1]);
    w.set_active(12, false);
    assert!(!w.is_active_in_hierarchy(1), "12 arrived and went dark");
}

#[test]
fn a_late_parent_lists_its_waiting_child_and_hides_it() {
    let mut w = late_parent();
    assert!(w.is_active_in_hierarchy(1));
    assert_eq!(w.children(3), vec![1]);
    assert!(!w.activity.is_unlisted(), "repaired, not marked");
    assert!(w.awaited.is_empty());
    w.set_active(3, false);
    assert!(!w.is_active_in_hierarchy(1));
    w.set_active(3, true);
    assert!(w.is_active_in_hierarchy(1));
}

#[test]
fn a_late_parent_turning_off_stamps_its_adopted_child() {
    let mut w = World::new();
    let mut cursor = w.change_cursor::<Activity>();
    w.insert_entity(orphan(1, 3));
    w.spawn("e".into());
    w.spawn("e".into());
    let _ = w.changes(&mut cursor);
    w.set_active(3, false);
    match w.changes(&mut cursor) {
        Changes::Ids(ids) => assert!(ids.contains(&1), "stamped: {ids:?}"),
        Changes::Rebuild => panic!("a flag change is ids, not a rebuild"),
    }
}

#[test]
fn a_late_parent_moving_carries_its_adopted_child() {
    let mut w = late_parent();
    let at = |w: &World| w.local_to_world(1).w_axis.truncate();
    assert_eq!(at(&w), Vec3::ZERO, "the kept matrix is warm");
    w.transform_mut(3).expect("live").position = Vec3::X;
    assert_eq!(at(&w), Vec3::X);
}

#[test]
fn a_late_parent_despawned_leaves_its_adopted_child_a_root() {
    let mut w = late_parent();
    w.despawn(3);
    assert_eq!(w.parent_id(1), None, "re-rooted, naming no dead id");
}

#[test]
fn a_document_naming_a_live_parent_is_listed_at_once() {
    let mut w = World::new();
    assert_eq!(w.spawn("e".into()), 1);
    w.insert_entity(orphan(2, 1));
    assert!(w.is_active_in_hierarchy(2));
    assert_eq!(w.children(1), vec![2]);
    w.set_active(1, false);
    assert!(!w.is_active_in_hierarchy(2));
    assert!(!w.activity.is_unlisted());
}

#[test]
fn a_parent_put_back_listing_its_child_lists_it_once() {
    // Undo puts a step's entities back in id order: a child can arrive before the
    // parent whose document lists it.
    let mut w = World::new();
    w.insert_entity(orphan(1, 2));
    let mut parent = Entity::new(2, "parent".into());
    parent.children = vec![1];
    w.restore_entity(parent, 0);
    assert_eq!(w.children(2), vec![1]);
    w.set_active(2, false);
    assert!(!w.is_active_in_hierarchy(1));
}

#[test]
fn a_waiting_child_that_left_is_not_adopted() {
    let mut w = World::new();
    w.insert_entity(orphan(1, 3));
    w.set_parent_id(1, None);
    w.spawn("e".into());
    assert_eq!(w.spawn("e".into()), 3);
    assert!(w.children(3).is_empty(), "1 no longer names 3");
}
