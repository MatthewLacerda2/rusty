//! The world-matrix walks on a malformed hierarchy (#991): a parent cycle (which
//! only a raw `set_parent_id`, e.g. a corrupt scene file, can write) ends the walk,
//! and a dangling parent reads as a root, whatever order the store fills in. A raw link
//! the parent's `children` lacks is the one case the store's downward walk cannot see
//! (`ecs::world_space::store`).

use glam::Vec3;

use crate::scene::Scene;

#[test]
fn a_parent_cycle_neither_hangs_nor_overflows_the_store() {
    let mut scene = Scene::new();
    let a = scene.add_entity("a".into());
    let b = scene.add_entity("b".into());
    scene.world.set_parent_id(a, Some(b));
    scene.world.set_parent_id(b, Some(a));
    assert!(scene.world.local_to_world(a).is_finite());
    assert!(scene.world.local_to_world(b).is_finite());
}

#[test]
fn a_dangling_parent_reads_as_a_root() {
    // A raw link to an id no longer in the world, as a corrupt file can write. (A
    // despawn under a well-formed parent is `ecs::world_space::queue_tests`'.)
    let mut scene = Scene::new();
    let gone = scene.add_entity("gone".into());
    let leaf = scene.add_entity("leaf".into());
    scene.world.transform_mut(gone).unwrap().position = Vec3::X * 10.0;
    scene.world.transform_mut(leaf).unwrap().position = Vec3::Y;
    assert_eq!(scene.world.world_position(leaf), Vec3::Y);
    scene.world.despawn(gone);
    scene.world.set_parent_id(leaf, Some(gone));
    let live = scene.world.world_position(leaf);
    let cached = scene.world.world_position(leaf);
    assert_eq!(live, Vec3::Y);
    assert_eq!(cached, live);
}

#[test]
fn the_store_fills_parents_first_whatever_the_id_order() {
    let mut scene = Scene::new();
    let child = scene.add_entity("child".into());
    let parent = scene.add_entity("parent".into());
    scene.world.transform_mut(parent).unwrap().position = Vec3::X;
    scene.world.transform_mut(child).unwrap().position = Vec3::Z;
    scene.set_parent(child, Some(parent)).unwrap();
    assert_eq!(scene.world.world_position(child), Vec3::new(1.0, 0.0, 1.0));
}

#[test]
fn a_failed_re_parent_keeps_the_child_under_its_old_parent() {
    // `pcall(Scene.SetParent, child, destroyedId)`: the call fails and changes nothing,
    // so moving the old parent still moves the child.
    let mut scene = Scene::new();
    let parent = scene.add_entity("parent".into());
    let child = scene.add_entity("child".into());
    let gone = scene.add_entity("gone".into());
    scene.set_parent(child, Some(parent)).unwrap();
    scene.world.despawn(gone);
    assert!(scene.set_parent(child, Some(gone)).is_err());
    assert_eq!(scene.world.parent_id(child), Some(parent));
    assert_eq!(scene.world.children(parent), vec![child]);
    assert_eq!(scene.world.world_position(child), Vec3::ZERO);
    scene.world.transform_mut(parent).unwrap().position = Vec3::X * 3.0;
    assert_eq!(scene.world.world_position(child), Vec3::X * 3.0);
}
