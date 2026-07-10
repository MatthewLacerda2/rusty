//! Tests for the per-frame world-matrix cache and its freshness contract (#331).

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
fn cache_matches_the_recursive_walk_over_a_deep_hierarchy() {
    let mut scene = Scene::new();
    let (gp, p, c) = nested_chain(&mut scene);

    scene.refresh_world_matrices();

    // Every entity's cached matrix equals the authoritative recursive computation.
    for id in [gp, p, c] {
        assert_eq!(scene.world_matrix(id), scene.compute_world_matrix(id));
    }
    // And the child accumulates the whole chain: 1 + 2 + 4 on X.
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
}

#[test]
fn a_refresh_makes_a_mid_tick_transform_write_observable() {
    // The freshness contract: `world_matrix` returns values as of the last refresh; the
    // refresh the render path runs each frame — after the sim tick settles — is what lets
    // rendering observe a transform a script wrote earlier in the same tick.
    let mut scene = Scene::new();
    let (gp, _p, c) = nested_chain(&mut scene);
    scene.refresh_world_matrices();
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));

    // A "script Update" moves the grandparent (+10 on X) mid-tick.
    scene.world.transform_mut(gp).unwrap().position = Vec3::new(11.0, 0.0, 0.0);

    // Until the next refresh the cache still reports the as-of-refresh value (contract),
    // while the live walk already sees the write.
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
    assert_eq!(
        scene.compute_world_matrix(c).w_axis.truncate(),
        Vec3::new(17.0, 0.0, 0.0)
    );

    // The render refresh makes the write observable through the cache too — the frame
    // renders the entity at its just-written position.
    scene.refresh_world_matrices();
    assert_eq!(world_translation(&scene, c), Vec3::new(17.0, 0.0, 0.0));
}

#[test]
fn a_cache_miss_falls_back_to_the_live_walk_never_stale_wrong() {
    // An entity spawned after the last refresh is absent from the store; reading it must
    // fall back to the recursive walk (correct), not return a stale/identity matrix.
    let mut scene = Scene::new();
    let (_gp, _p, c) = nested_chain(&mut scene);
    scene.refresh_world_matrices();

    let late = scene.add_entity("spawned-after-refresh".into());
    scene.world.transform_mut(late).unwrap().position = Vec3::new(5.0, 6.0, 7.0);

    // `late` was never cached, yet the read is correct via fallback.
    assert_eq!(world_translation(&scene, late), Vec3::new(5.0, 6.0, 7.0));
    // The previously cached entity is unaffected.
    assert_eq!(world_translation(&scene, c), Vec3::new(7.0, 0.0, 0.0));
}
