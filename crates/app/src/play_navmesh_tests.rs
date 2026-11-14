//! Play starts from the scene's saved navmesh (#1887): a scene saved with one and
//! loaded back enters Play without a bake, a stale one is baked instead, and a
//! play-mode `Scene.Load` and Stop restore it too, each once. A path budget set in
//! Play ends with it (#2087).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::GameWorld;
use rusty_core::input::InputState;
use rusty_navigation::test_support::{add_box, add_floor};
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;
/// The floor a marked file gives its first span: no bake makes it.
const MARK: f32 = 0.125;

/// A floor and a crate saved to disk with their navmesh, its first span's floor
/// overwritten by [`MARK`] (the fingerprint still matches), and loaded back.
fn saved_and_loaded(name: &str, stale: bool) -> GameWorld {
    let path = saved(name, stale);
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    world(loaded)
}

/// The path of a scene saved as [`saved_and_loaded`] describes.
fn saved(name: &str, stale: bool) -> String {
    let mut scene = Scene::new();
    add_floor(&mut scene, -4.0, 4.0, -4.0, 4.0);
    let crate_id = add_box(&mut scene, Vec3::ZERO, Vec3::ONE);
    let mut bytes = NavigationGraph::from_scene(&scene).bake_to_bytes(&scene);
    let starts = u32::from_le_bytes(bytes[44..48].try_into().unwrap()) as usize;
    let at = 48 + starts * 4 + 4;
    bytes[at..at + 4].copy_from_slice(&MARK.to_le_bytes());
    scene.navmesh.bytes = Some(bytes.into());
    if stale {
        scene.world.transform_mut(crate_id).unwrap().position.x += 2.0;
    }
    let dir = rusty_test_support::temp::dir().join("rusty_play_navmesh");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name).to_string_lossy().into_owned();
    scene.save_to_file(&path).unwrap();
    path
}

fn world(scene: Scene) -> GameWorld {
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

#[test]
fn a_saved_navmesh_enters_play_without_a_bake() {
    let mut gw = saved_and_loaded("fresh.scene", false);
    gw.set_playing(true);
    gw.tick(DT);
    assert_eq!(gw.nav().borrow().spans[0].y, MARK, "restored, not baked");
}

#[test]
fn a_stale_saved_navmesh_is_baked_instead() {
    let mut gw = saved_and_loaded("stale.scene", true);
    gw.set_playing(true);
    gw.tick(DT);
    let nav = gw.nav().borrow();
    assert!(!nav.spans.is_empty());
    assert_ne!(nav.spans[0].y, MARK, "the crate moved: baked in full");
}

/// A play-mode `Scene.Load` makes the new scene's navmesh once, in the load step:
/// restored from a matching file (one generation, no bake), and the next tick only
/// syncs, where it used to bake a second time.
#[test]
fn a_play_mode_load_restores_the_saved_navmesh_once() {
    let target = saved("load_target.scene", false);
    let mut floor = Scene::new();
    add_floor(&mut floor, -2.0, 2.0, -2.0, 2.0);
    let mut gw = world(floor);
    gw.set_playing(true);
    gw.tick(DT);
    let before = gw.nav().borrow().bake_generation;
    gw.scene().borrow_mut().request_load(target);
    gw.tick(DT);
    gw.tick(DT);
    let nav = gw.nav().borrow();
    assert_eq!(nav.spans[0].y, MARK, "restored, not baked");
    assert_eq!(nav.bake_generation, before + 1, "made once");
}

/// Stop puts the edit scene back and restores its saved navmesh, not a bake.
#[test]
fn stop_restores_the_saved_navmesh() {
    let mut gw = saved_and_loaded("stop.scene", false);
    gw.set_playing(true);
    gw.tick(DT);
    gw.nav().borrow_mut().bake(&gw.scene().borrow());
    assert_ne!(gw.nav().borrow().spans[0].y, MARK, "a bake in Play");
    gw.set_playing(false);
    gw.tick(DT);
    assert_eq!(gw.nav().borrow().spans[0].y, MARK, "restored on Stop");
}

/// Boot (`NavigationGraph::from_scene`, the player's and the harness's path)
/// restores the saved navmesh too.
#[test]
fn boot_restores_the_saved_navmesh() {
    let path = saved("boot.scene", false);
    let mut scene = Scene::new();
    scene.load_from_file(&path).unwrap();
    let nav = NavigationGraph::from_scene(&scene);
    assert_eq!(nav.spans[0].y, MARK, "restored at boot");
}

#[test]
fn a_play_mode_path_budget_ends_with_play() {
    let mut gw = world(Scene::new());
    let budget = |gw: &GameWorld| gw.nav().borrow().pathfinding_iterations_per_frame();
    let set = |gw: &GameWorld| gw.nav().borrow().set_pathfinding_iterations_per_frame(64);
    let default = rusty_navigation::DEFAULT_ITERATIONS_PER_FRAME;
    gw.set_playing(true);
    gw.tick(DT);
    set(&gw); // Play 1's script sets it
    gw.set_playing(false);
    gw.tick(DT);
    assert_eq!(budget(&gw), default, "Stop reverts it");
    set(&gw);
    gw.set_playing(true);
    gw.tick(DT);
    assert_eq!(budget(&gw), default, "Play 2 starts at the default");
}
