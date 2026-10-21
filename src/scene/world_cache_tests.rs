//! The scene's world matrices read the ECS store, which is never stale (#331, #1170).

use glam::Vec3;

use crate::scene::Scene;

/// Build `grandparent -> parent -> child`, each offset on X, and return their ids.
fn nested_chain(scene: &mut Scene) -> (u32, u32, u32) {
    let gp = scene.add_entity("grandparent".into());
    let p = scene.add_entity("parent".into());
    let c = scene.add_entity("child".into());
    scene.world.transform_mut(gp).unwrap().position = Vec3::new(1.0, 0.0, 0.0);
    scene.world.transform_mut(p).unwrap().position = Vec3::new(2.0, 0.0, 0.0);
    scene.world.transform_mut(c).unwrap().position = Vec3::new(4.0, 0.0, 0.0);
    scene.set_parent(p, Some(gp)).unwrap();
    scene.set_parent(c, Some(p)).unwrap();
    (gp, p, c)
}

fn world_translation(scene: &Scene, id: u32) -> Vec3 {
    scene.world_matrix(id).w_axis.truncate()
}

#[test]
fn a_deep_chain_accumulates_every_ancestor() {
    let mut scene = Scene::new();
    let (gp, p, c) = nested_chain(&mut scene);
    assert_eq!(world_translation(&scene, gp), Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(world_translation(&scene, p), Vec3::new(3.0, 0.0, 0.0));
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
}

#[test]
fn a_mid_tick_write_is_seen_by_the_next_read() {
    // The old per-frame cache answered as of its last refresh; the store answers now.
    let mut scene = Scene::new();
    let (gp, _p, c) = nested_chain(&mut scene);
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
    scene.world.transform_mut(gp).unwrap().position = Vec3::new(11.0, 0.0, 0.0);
    assert_eq!(world_translation(&scene, c), Vec3::new(17.0, 0.0, 0.0));
}

#[test]
fn re_parenting_moves_the_subtree() {
    let mut scene = Scene::new();
    let (gp, p, c) = nested_chain(&mut scene);
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
    scene.set_parent(p, None).unwrap();
    assert_eq!(world_translation(&scene, c), Vec3::new(6.0, 0.0, 0.0));
    scene.set_parent(p, Some(gp)).unwrap();
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
}

#[test]
fn a_despawned_entity_is_never_answered_from_its_old_slot() {
    let mut scene = Scene::new();
    let (gp, p, c) = nested_chain(&mut scene);
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
    scene.world.despawn(c);
    assert_eq!(scene.world_matrix(c), glam::Mat4::IDENTITY);
    assert_eq!(world_translation(&scene, p), Vec3::new(3.0, 0.0, 0.0));
    assert_eq!(world_translation(&scene, gp), Vec3::new(1.0, 0.0, 0.0));
}

#[test]
fn an_id_reused_after_a_clear_reads_the_new_entity_not_the_old_one() {
    // A scene load clears the world and restarts ids at 1, so the next spawns reuse ids.
    let mut scene = Scene::new();
    let (gp, _p, c) = nested_chain(&mut scene);
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));

    scene.world.clear();
    let a = scene.add_entity("new-a".into());
    let _b = scene.add_entity("new-b".into());
    let reused = scene.add_entity("new-c".into());
    assert_eq!((a, reused), (gp, c), "ids restart at 1 after a clear");
    scene.world.transform_mut(reused).unwrap().position = Vec3::new(0.0, 9.0, 0.0);
    assert_eq!(world_translation(&scene, reused), Vec3::new(0.0, 9.0, 0.0));
}

/// `id`'s world matrix from scratch: its local Transform under every ancestor's.
fn from_scratch(scene: &Scene, id: u32) -> glam::Mat4 {
    scene
        .world
        .self_and_ancestors(id)
        .filter_map(|c| scene.world.transform(c).map(|t| t.to_matrix()))
        .fold(glam::Mat4::IDENTITY, |below, local| local * below)
}

#[test]
fn the_store_matches_a_from_scratch_walk_across_churn() {
    // Spawn, reparent, move and despawn, reading in between: every read agrees with the
    // walk from scratch, whatever the earlier reads left clean.
    let mut scene = Scene::new();
    let mut live: Vec<u32> = Vec::new();
    for frame in 0..40u32 {
        let id = scene.add_entity(format!("e{frame}"));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(frame as f32, 1.0, 0.0);
        if let Some(&parent) = live.get((frame as usize * 7) % live.len().max(1)) {
            scene.set_parent(id, Some(parent)).unwrap();
        }
        live.push(id);
        let mover = live[(frame as usize * 3) % live.len()];
        scene.world.transform_mut(mover).unwrap().position.y += 0.5;
        if frame % 3 == 2 {
            let gone = live.remove((frame as usize * 5) % live.len());
            scene.world.despawn(gone);
            assert_eq!(scene.world_matrix(gone), glam::Mat4::IDENTITY);
        }
        for &id in scene.world.ids() {
            assert_eq!(scene.world_matrix(id), from_scratch(&scene, id), "id {id}");
        }
    }
}
