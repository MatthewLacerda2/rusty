//! Tests for the per-frame world-matrix cache (#331): the O(N) `rebuild_world_matrices`
//! fill, the `world_matrix` accessor, and the freshness contract that a transform write
//! observed by a rebuild is seen by every consumer in the same tick.

use glam::Vec3;

use crate::scene::Scene;

/// Spawn an entity at a pure translation and return its id.
fn spawn_at(scene: &mut Scene, name: &str, pos: Vec3) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.get_entity_mut(id).unwrap().transform.position = pos;
    id
}

/// A grandparent → parent → child chain of pure translations: the cached world matrix
/// must equal the fresh recursive walk for every entity, and the leaf's world position
/// is the sum of the chain's local offsets.
#[test]
fn cached_matches_recursive_for_nested_chain() {
    let mut scene = Scene::new();
    let gp = spawn_at(&mut scene, "gp", Vec3::new(1.0, 0.0, 0.0));
    let parent = spawn_at(&mut scene, "parent", Vec3::new(0.0, 2.0, 0.0));
    let child = spawn_at(&mut scene, "child", Vec3::new(0.0, 0.0, 3.0));
    scene.set_parent(parent, Some(gp)).unwrap();
    scene.set_parent(child, Some(parent)).unwrap();

    scene.rebuild_world_matrices();

    for id in [gp, parent, child] {
        assert_eq!(scene.world_matrix(id), scene.compute_world_matrix(id));
    }
    // Pure translations sum down the chain.
    let child_world = scene.world_matrix(child).col(3).truncate();
    assert_eq!(child_world, Vec3::new(1.0, 2.0, 3.0));
}

/// A parent shared by two children is resolved consistently for both — the memoized
/// fill computes the shared ancestor once yet every child reads the same result.
#[test]
fn shared_parent_resolved_for_every_child() {
    let mut scene = Scene::new();
    let parent = spawn_at(&mut scene, "parent", Vec3::new(5.0, 0.0, 0.0));
    let a = spawn_at(&mut scene, "a", Vec3::new(0.0, 1.0, 0.0));
    let b = spawn_at(&mut scene, "b", Vec3::new(0.0, -1.0, 0.0));
    scene.set_parent(a, Some(parent)).unwrap();
    scene.set_parent(b, Some(parent)).unwrap();

    scene.rebuild_world_matrices();

    assert_eq!(scene.world_matrix(a).col(3).truncate(), Vec3::new(5.0, 1.0, 0.0));
    assert_eq!(scene.world_matrix(b).col(3).truncate(), Vec3::new(5.0, -1.0, 0.0));
}

/// The freshness contract: a transform write followed by a rebuild is observed by
/// `world_matrix` in the same tick. This is exactly the "a mid-frame script write during
/// Update is seen by render this frame" guarantee — render rebuilds at its start, so any
/// write before it lands; here we move the parent and prove the child's world follows.
#[test]
fn rebuild_observes_latest_transform_write() {
    let mut scene = Scene::new();
    let parent = spawn_at(&mut scene, "parent", Vec3::new(0.0, 0.0, 0.0));
    let child = spawn_at(&mut scene, "child", Vec3::new(0.0, 0.0, 1.0));
    scene.set_parent(child, Some(parent)).unwrap();

    scene.rebuild_world_matrices();
    assert_eq!(scene.world_matrix(child).col(3).truncate(), Vec3::new(0.0, 0.0, 1.0));

    // Move the parent (the "mid-frame write"), then rebuild as render/physics would.
    scene.get_entity_mut(parent).unwrap().transform.position = Vec3::new(10.0, 0.0, 0.0);
    scene.rebuild_world_matrices();

    assert_eq!(scene.world_matrix(child).col(3).truncate(), Vec3::new(10.0, 0.0, 1.0));
}

/// A cache miss (entity spawned after the last rebuild, or no rebuild at all) falls back
/// to the fresh recursive walk, so `world_matrix` is never wrong — only uncached.
#[test]
fn cache_miss_falls_back_to_fresh_walk() {
    let mut scene = Scene::new();
    let a = spawn_at(&mut scene, "a", Vec3::new(1.0, 2.0, 3.0));
    scene.rebuild_world_matrices();

    // Spawned after the rebuild: not in the cache, must still resolve correctly.
    let b = spawn_at(&mut scene, "b", Vec3::new(4.0, 5.0, 6.0));

    assert_eq!(scene.world_matrix(a).col(3).truncate(), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(scene.world_matrix(b).col(3).truncate(), Vec3::new(4.0, 5.0, 6.0));
    assert_eq!(scene.world_matrix(b), scene.compute_world_matrix(b));
}
