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
