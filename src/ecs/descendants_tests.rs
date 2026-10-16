//! src/ecs/descendants_tests.rs — `World::self_and_descendants` (#991): pre-order in
//! declared order, and a malformed `children` list ends the walk.

use crate::ecs::World;

fn walk(w: &World, id: u32) -> Vec<u32> {
    w.self_and_descendants(id).collect()
}

#[test]
fn descendants_run_pre_order_in_declared_order() {
    let mut w = World::new();
    let [root, a, a1, b] = ["root", "a", "a1", "b"].map(|n| w.spawn(n.into()));
    for (child, parent) in [(a, root), (a1, a), (b, root)] {
        w.set_parent_id(child, Some(parent));
        w.add_child(parent, child);
    }
    assert_eq!(walk(&w, root), vec![root, a, a1, b]);
    assert_eq!(walk(&w, a), vec![a, a1]);
    let pruned: Vec<u32> = w.self_and_descendants_while(root, |id| id != a).collect();
    assert_eq!(
        pruned,
        vec![root, a, b],
        "a is yielded, its children are not"
    );
}

#[test]
fn a_children_cycle_visits_each_id_once() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    w.add_child(a, b);
    w.add_child(b, a);
    w.add_child(a, b); // listed twice
    assert_eq!(walk(&w, a), vec![a, b]);
}

#[test]
fn dead_ids_are_skipped() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let gone = w.spawn("gone".into());
    w.add_child(a, gone);
    w.despawn(gone);
    assert_eq!(walk(&w, a), vec![a]);
    assert!(walk(&w, gone).is_empty());
}
