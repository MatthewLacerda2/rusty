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

#[test]
fn a_hierarchy_path_reads_back_to_its_entity_among_same_names() {
    let mut w = World::new();
    let arena = w.spawn("Arena".into());
    let crates = [0; 5].map(|_| w.spawn("Crate".into()));
    let slash = w.spawn("a/b".into());
    for child in crates.into_iter().chain([slash]) {
        w.set_parent_id(child, Some(arena));
        w.add_child(arena, child);
    }
    let dup = w.spawn("Arena".into());
    for id in crates.into_iter().chain([arena, slash, dup]) {
        let path = w.path_of(id).unwrap();
        assert_eq!(w.find_path(&path), Some(id), "{path:?}");
    }
    assert_eq!(w.find_child(arena, "Crate"), Some(crates[0]));
    assert_eq!(w.path_of(99), None);
}
