//! src/ecs/changes/seam_tests.rs — what the consumers of #1706–#1710 are briefed to
//! rely on (#1705): a write that changed nothing names nothing, a move or re-parent
//! names the whole subtree on `WorldPose`, a derived box is not a collider edit, and
//! restoring a live entity names it like any overwrite.

use crate::components::{ColliderComponent, TransformComponent};
use crate::ecs::{Activity, Changes, World, WorldPose};

/// root -> mid -> leaf, plus a separate root `other`.
fn chain() -> (World, [u32; 4]) {
    let mut w = World::new();
    let [root, mid, leaf, other] = ["root", "mid", "leaf", "other"].map(|n| w.spawn(n.into()));
    for (child, parent) in [(mid, root), (leaf, mid)] {
        w.set_parent_id(child, Some(parent));
        w.add_child(parent, child);
    }
    (w, [root, mid, leaf, other])
}

#[test]
fn a_transform_landing_its_old_value_names_nothing() {
    let (mut w, [root, ..]) = chain();
    let mut local = w.change_cursor::<TransformComponent>();
    let mut poses = w.change_cursor::<WorldPose>();
    let held = w.transform(root).expect("live").position.x;
    w.transform_mut(root).expect("live").position.x = held; // physics' write-back of a sleeper
    assert_eq!(w.changes(&mut local), Changes::Ids(vec![]));
    assert_eq!(w.changes(&mut poses), Changes::Ids(vec![]));
}

#[test]
fn a_move_names_the_subtree_on_world_pose_and_only_the_entity_on_transform() {
    let (mut w, [root, mid, leaf, _]) = chain();
    let mut local = w.change_cursor::<TransformComponent>();
    let mut poses = w.change_cursor::<WorldPose>();
    w.transform_mut(mid).expect("live").position.y = 2.0;
    w.transform_mut(mid).expect("live").position.y = 3.0; // still unread: no second walk
    assert_eq!(w.changes(&mut local), Changes::Ids(vec![mid]));
    assert_eq!(w.changes(&mut poses), Changes::Ids(vec![mid, leaf]));
    w.transform_mut(root).expect("live").position.y = 1.0;
    assert_eq!(w.changes(&mut poses), Changes::Ids(vec![root, mid, leaf]));
}

#[test]
fn a_re_parent_names_the_subtree_on_world_pose_and_activity() {
    let (mut w, [root, mid, leaf, other]) = chain();
    w.set_active(other, false);
    let mut local = w.change_cursor::<TransformComponent>();
    let mut poses = w.change_cursor::<WorldPose>();
    let mut activity = w.change_cursor::<Activity>();
    w.remove_child(root, mid);
    w.set_parent_id(mid, Some(other)); // keeps its local Transform
    w.add_child(other, mid);
    assert_eq!(w.changes(&mut local), Changes::Ids(vec![]));
    assert_eq!(w.changes(&mut poses), Changes::Ids(vec![mid, leaf]));
    assert_eq!(w.changes(&mut activity), Changes::Ids(vec![mid, leaf]));
    assert!(!w.is_active_in_hierarchy(leaf));
}

#[test]
fn a_refreshed_collider_box_is_not_a_collider_edit() {
    let (mut w, [root, mid, ..]) = chain();
    w.set_collider(
        mid,
        Some(ColliderComponent::from_mesh_bounds(
            false,
            -glam::Vec3::ONE,
            glam::Vec3::ONE,
        )),
    );
    let mut colliders = w.change_cursor::<ColliderComponent>();
    w.transform_mut(root).expect("live").position.x = 5.0;
    w.flush_bounds();
    assert_ne!(w.collider(mid).expect("set").aabb_min, glam::Vec3::ZERO);
    assert_eq!(w.changes(&mut colliders), Changes::Ids(vec![]));
}

#[test]
fn restoring_a_live_entity_names_it_and_its_subtree() {
    let (mut w, [root, mid, leaf, _]) = chain();
    let mut local = w.change_cursor::<TransformComponent>();
    let mut poses = w.change_cursor::<WorldPose>();
    let before = w.record(root).expect("live");
    w.transform_mut(root).expect("live").position.x = 4.0;
    let _ = (w.changes(&mut local), w.changes(&mut poses));
    w.restore_entity(before.doc, before.seq); // an Undo step's own rewind is `scene::undo`'s
    assert_eq!(w.changes(&mut local), Changes::Ids(vec![root]));
    assert_eq!(w.changes(&mut poses), Changes::Ids(vec![root, mid, leaf]));
}

#[test]
fn change_stats_name_each_opened_channel_and_count_what_it_logged() {
    let (mut w, [root, ..]) = chain();
    let mut poses = w.change_cursor::<WorldPose>();
    w.transform_mut(root).expect("live").position.x = 1.0;
    let _ = w.changes(&mut poses);
    w.transform_mut(root).expect("live").position.x = 2.0;
    let names: Vec<_> = w
        .change_stats()
        .iter()
        .map(|s| (s.name, s.logged))
        .collect();
    assert_eq!(names, [("static_set", 0), ("world_pose", 6)]);
}
