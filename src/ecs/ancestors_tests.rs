//! src/ecs/ancestors_tests.rs — `World::ancestors` (#939): nearest first, and a
//! parent cycle (which only a raw `set_parent_id` can write) ends the walk.

use crate::ecs::World;

#[test]
fn ancestors_run_nearest_first_and_exclude_self() {
    let mut w = World::new();
    let root = w.spawn("root".into());
    let mid = w.spawn("mid".into());
    let leaf = w.spawn("leaf".into());
    w.set_parent_id(mid, Some(root));
    w.set_parent_id(leaf, Some(mid));
    assert_eq!(w.ancestors(leaf).collect::<Vec<_>>(), vec![mid, root]);
    assert_eq!(
        w.self_and_ancestors(leaf).collect::<Vec<_>>(),
        vec![leaf, mid, root]
    );
    assert_eq!(w.ancestors(root).count(), 0);
}

#[test]
fn a_parent_cycle_ends_the_walk() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    w.set_parent_id(a, Some(b));
    w.set_parent_id(b, Some(a));
    assert_eq!(w.ancestors(a).count(), w.len());
}

#[test]
fn a_dangling_parent_ends_the_walk() {
    let mut w = World::new();
    let gone = w.spawn("gone".into());
    let leaf = w.spawn("leaf".into());
    w.set_parent_id(leaf, Some(gone));
    w.despawn(gone);
    assert_eq!(w.ancestors(leaf).count(), 0);
}

#[test]
fn the_world_matrix_walk_survives_a_parent_cycle() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    w.set_parent_id(a, Some(b));
    w.set_parent_id(b, Some(a));
    assert!(w.local_to_world(a).is_finite());
}
