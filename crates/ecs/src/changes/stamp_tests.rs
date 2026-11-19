//! crates/ecs/src/changes/stamp_tests.rs — the subtree stamps' kept walk (`stamp.rs`): a
//! stamp below an id names its whole subtree once, a malformed child list included,
//! and a warm walk forgets the ids the walk before it reached.

use crate::{Activity, Changes, World, WorldPose};

/// root -> {a -> a1, b}, with `b` listing `root` back and `a` listed twice.
fn tree() -> (World, [u32; 5]) {
    let mut w = World::new();
    let ids = ["root", "a", "a1", "b", "other"].map(|n| w.spawn(n.into()));
    let [root, a, a1, b, _] = ids;
    for (parent, child) in [(root, a), (a, a1), (root, b), (b, root), (root, a)] {
        w.add_child(parent, child);
    }
    (w, ids)
}

#[test]
fn a_pose_stamp_names_the_subtree_once_and_again_after_a_read() {
    let (mut w, [root, a, a1, b, other]) = tree();
    let mut poses = w.change_cursor::<WorldPose>();
    w.stamp_pose_below(root);
    assert_eq!(w.changes(&mut poses), Changes::Ids(vec![root, a, a1, b]));
    w.stamp_pose_below(other);
    assert_eq!(w.changes(&mut poses), Changes::Ids(vec![other]));
    for _ in 0..3 {
        w.stamp_pose_below(a); // a warm walk reaches ids the last one did
        assert_eq!(w.changes(&mut poses), Changes::Ids(vec![a, a1]));
    }
}

#[test]
fn a_hierarchy_stamp_names_the_subtree_on_activity() {
    let (mut w, [root, a, a1, b, _]) = tree();
    let mut activity = w.change_cursor::<Activity>();
    w.stamp_hierarchy_below(a);
    assert_eq!(w.changes(&mut activity), Changes::Ids(vec![a, a1]));
    w.stamp_hierarchy_below(root);
    assert_eq!(w.changes(&mut activity), Changes::Ids(vec![root, a, a1, b]));
}
