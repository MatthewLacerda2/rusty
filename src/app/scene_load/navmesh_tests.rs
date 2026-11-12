//! A play-mode load keeps the level's current navmesh when a `DontDestroyOnLoad`
//! survivor moves one of its collider ids (#2127): the saved `<scene>.navmesh` on a
//! `Scene.Load`, and the one `Scene.LoadAsync` baked in the background, are
//! restored, so no bake runs on the swap tick.

use std::sync::Arc;

use glam::Vec3;

use super::fixture::world_at;
use super::GameWorld;
use crate::core::paths;
use crate::navigation::test_support::{add_box, add_floor};
use crate::navigation::{NavigationGraph, Start};
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// `start.scene` (`Keeper` = 1, `Other` = 2) and `level.scene` (a floor collider = 1
/// and a crate = 2) in a temp dir, the level's navmesh saved beside it when `saved`.
fn scenes(tag: &str, saved: bool) -> (String, String) {
    let dir = crate::test_temp::dir().join(format!("rusty_2127_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = |name: &str| paths::to_slash(&dir.join(name));
    let mut start = Scene::new();
    start.add_entity("Keeper".into());
    start.add_entity("Other".into());
    start.save_to_file(&path("start.scene")).unwrap();
    let mut level = Scene::new();
    add_floor(&mut level, 0.0, 6.0, 0.0, 6.0);
    add_box(
        &mut level,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(3.0, 1.0, 3.0),
    );
    if saved {
        let bytes = NavigationGraph::baked_bytes(&level);
        level.put_baked_navmesh(Arc::from(bytes));
    }
    level.save_to_file(&path("level.scene")).unwrap();
    (path("start.scene"), path("level.scene"))
}

/// Play `start`, keep `Keeper` (id 1, the level floor's id) across the next load.
fn playing_with_a_survivor(start: &str) -> GameWorld {
    let (mut gw, _) = world_at(start);
    gw.set_playing(true);
    gw.tick(DT);
    let mut scene = gw.scene().borrow_mut();
    let keeper = scene.find_entity_by_name("Keeper").unwrap();
    assert_eq!(keeper, 1, "the survivor holds the level floor's id");
    assert!(scene.dont_destroy_on_load(keeper));
    drop(scene);
    gw
}

/// The level is in, its floor moved off id 1, and how the navmesh started.
fn after_the_swap(gw: &GameWorld) -> Option<Start> {
    let scene = gw.scene().borrow();
    assert_eq!(scene.find_entity_by_name("Keeper"), Some(1));
    let colliders = scene.entity_ids().into_iter();
    let mut colliders: Vec<_> = colliders
        .filter(|&id| scene.world.collider(id).is_some())
        .collect();
    colliders.sort_unstable();
    assert_eq!(
        colliders,
        vec![2, 3],
        "the level is in, its floor moved off id 1"
    );
    gw.nav().borrow().last_start()
}

#[test]
fn a_load_with_a_survivor_restores_the_saved_navmesh() {
    let (start, level) = scenes("load", true);
    let mut gw = playing_with_a_survivor(&start);
    gw.scene().borrow_mut().request_load(level);
    gw.tick(DT);
    assert_eq!(after_the_swap(&gw), Some(Start::Restored));
}

#[test]
fn a_load_async_with_a_survivor_restores_its_background_bake() {
    let (start, level) = scenes("async", false);
    let mut gw = playing_with_a_survivor(&start);
    let op = gw
        .script_manager()
        .eval(&format!("return Scene.LoadAsync('{level}')"));
    let op = op.unwrap();
    gw.tick(DT);
    let done = format!("return Scene.GetLoadStatus({op})");
    assert_eq!(gw.script_manager().eval(&done).unwrap(), "done");
    assert_eq!(
        after_the_swap(&gw),
        Some(Start::Restored),
        "no bake on the swap tick"
    );
}
