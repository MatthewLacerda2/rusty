use crate::{Activity, Changes, Cut, World, WorldPose};

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
        w.stamp_pose_below(a);
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

#[test]
fn a_cut_names_the_subtree_and_never_opens_a_log_nobody_reads() {
    let (mut w, [root, a, a1, b, other]) = tree();
    w.cut_pose(root);
    let names: Vec<_> = w.change_stats().iter().map(|s| s.name).collect();
    assert_eq!(names, ["static_set"], "no reader: nothing kept");
    let mut cuts = w.change_cursor::<Cut>();
    w.cut_pose(a);
    assert_eq!(w.changes(&mut cuts), Changes::Ids(vec![a, a1]));
    w.cut_pose(root);
    w.cut_pose(other);
    assert_eq!(
        w.changes(&mut cuts),
        Changes::Ids(vec![root, a, a1, b, other])
    );
    assert!(w.changes(&mut cuts).is_empty(), "read: nothing owed");
}
