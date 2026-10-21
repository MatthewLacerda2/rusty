//! The world-matrix walks on a malformed hierarchy (#991): a parent cycle (which
//! only a raw `set_parent_id`, e.g. a corrupt scene file, can write) ends the walk,
//! and a dangling parent reads as a root, whatever order the store fills in.

use glam::Vec3;

use crate::scene::Scene;

#[test]
fn a_parent_cycle_neither_hangs_nor_overflows_the_store() {
    let mut scene = Scene::new();
    let a = scene.add_entity("a".into());
    let b = scene.add_entity("b".into());
    scene.world.set_parent_id(a, Some(b));
    scene.world.set_parent_id(b, Some(a));
    assert!(scene.world_matrix(a).is_finite());
    assert!(scene.compute_world_matrix(b).is_finite());
}

#[test]
fn a_dangling_parent_reads_as_a_root() {
    let mut scene = Scene::new();
    let gone = scene.add_entity("gone".into());
    let leaf = scene.add_entity("leaf".into());
    scene.world.transform_mut(gone).unwrap().position = Vec3::X * 10.0;
    scene.world.transform_mut(leaf).unwrap().position = Vec3::Y;
    scene.world.set_parent_id(leaf, Some(gone));
    assert_eq!(
        scene.world_matrix(leaf).w_axis.truncate(),
        Vec3::new(10.0, 1.0, 0.0)
    );
    scene.world.despawn(gone);
    let live = scene.compute_world_matrix(leaf).w_axis.truncate();
    let cached = scene.world_matrix(leaf).w_axis.truncate();
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
    assert_eq!(scene.world_matrix(child), scene.compute_world_matrix(child));
    assert_eq!(
        scene.world_matrix(child).w_axis.truncate(),
        Vec3::new(1.0, 0.0, 1.0)
    );
}
