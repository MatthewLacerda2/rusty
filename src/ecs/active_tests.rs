//! src/ecs/active_tests.rs — `activeSelf` vs `activeInHierarchy` (#1363): an
//! inactive ancestor hides the whole subtree without touching its own flags.

use crate::ecs::World;

fn chain(w: &mut World) -> (u32, u32, u32) {
    let root = w.spawn("root".into());
    let pivot = w.spawn("pivot".into());
    let gun = w.spawn("gun".into());
    w.set_parent_id(pivot, Some(root));
    w.set_parent_id(gun, Some(pivot));
    (root, pivot, gun)
}

#[test]
fn an_inactive_ancestor_hides_the_subtree_but_keeps_its_flags() {
    let mut w = World::new();
    let (root, pivot, gun) = chain(&mut w);
    assert!(w.is_active_in_hierarchy(gun));
    w.set_active(pivot, false);
    assert!(!w.is_active_in_hierarchy(gun) && !w.is_active_in_hierarchy(pivot));
    assert!(
        w.is_active_in_hierarchy(root),
        "an ancestor is not hidden by a child"
    );
    assert!(w.is_active_self(gun), "the child's own flag is untouched");
    w.set_active(pivot, true);
    assert!(w.is_active_in_hierarchy(gun));
    w.set_active(root, false);
    assert!(
        !w.is_active_in_hierarchy(gun),
        "any ancestor, not only the parent"
    );
}

#[test]
fn a_dead_id_or_a_parent_cycle_answers_without_hanging() {
    let mut w = World::new();
    assert!(!w.is_active_in_hierarchy(7) && !w.is_active_self(7));
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    w.set_parent_id(a, Some(b));
    w.set_parent_id(b, Some(a));
    assert!(w.is_active_in_hierarchy(a));
    w.set_active(b, false);
    assert!(!w.is_active_in_hierarchy(a));
}

#[test]
fn find_by_name_skips_an_entity_under_an_inactive_parent() {
    let mut w = World::new();
    let (_, pivot, gun) = chain(&mut w);
    assert_eq!(w.find_by_name("gun"), Some(gun));
    w.set_active(pivot, false);
    assert_eq!(w.find_by_name("gun"), None, "Unity's GameObject.Find");
}

// The kept answers (#1388): every write that can change one is seen by the next read.

#[test]
fn a_kept_answer_follows_a_reparent_and_a_parent_despawn() {
    let mut w = World::new();
    let (root, pivot, gun) = chain(&mut w);
    let other = w.spawn("other".into());
    w.set_active(other, false);
    assert!(w.is_active_in_hierarchy(gun), "answered, so kept");
    w.set_parent_id(pivot, Some(other));
    assert!(
        !w.is_active_in_hierarchy(gun),
        "moved under an inactive parent"
    );
    w.set_parent_id(pivot, Some(root));
    w.set_active(root, false);
    assert!(!w.is_active_in_hierarchy(gun));
    w.despawn(root);
    assert!(
        w.is_active_in_hierarchy(gun),
        "a dead parent ends the chain"
    );
}

#[test]
fn a_kept_answer_follows_an_overwritten_or_inserted_document() {
    let mut w = World::new();
    let (_, pivot, gun) = chain(&mut w);
    assert!(w.is_active_in_hierarchy(gun));
    let mut doc = w.entity_document(pivot).expect("live");
    doc.active = false;
    assert!(w.overwrite(doc));
    assert!(
        !w.is_active_in_hierarchy(gun),
        "the overwrite cleared the pivot"
    );
    let orphan = w.spawn("orphan".into());
    w.set_parent_id(orphan, Some(99));
    assert!(w.is_active_in_hierarchy(orphan));
    let mut parent = crate::components::Entity::new(99, "late".into());
    parent.active = false;
    w.insert_entity(parent);
    assert!(
        !w.is_active_in_hierarchy(orphan),
        "the parent it named arrived"
    );
}

#[test]
fn a_longer_parent_cycle_answers_the_same_whichever_node_is_asked_first() {
    let mut w = World::new();
    let [a, b, c] = ["a", "b", "c"].map(|n| w.spawn(n.into()));
    w.set_parent_id(a, Some(b));
    w.set_parent_id(b, Some(c));
    w.set_parent_id(c, Some(a));
    w.set_active(b, false);
    assert!(!w.is_active_in_hierarchy(a));
    assert!(
        !w.is_active_in_hierarchy(c),
        "the whole loop, not the part above a"
    );
}

#[test]
fn a_child_asked_after_its_inactive_parent_reads_the_kept_answer() {
    let mut w = World::new();
    let (root, _, gun) = chain(&mut w);
    w.set_active(root, false);
    assert!(!w.is_active_in_hierarchy(root), "the root answered first");
    assert!(
        !w.is_active_in_hierarchy(gun),
        "its climb stops at the kept root"
    );
}
