//! Play starts from the scene's saved navmesh (#1887): a scene saved with one and
//! loaded back enters Play without a bake, and a stale one is baked instead.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::test_support::{add_box, add_floor};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;
/// The floor a marked file gives its first span: no bake makes it.
const MARK: f32 = 0.125;

/// A floor and a crate saved to disk with their navmesh, its first span's floor
/// overwritten by [`MARK`] (the fingerprint still matches), and loaded back.
fn saved_and_loaded(name: &str, stale: bool) -> GameWorld {
    let mut scene = Scene::new();
    add_floor(&mut scene, -4.0, 4.0, -4.0, 4.0);
    let crate_id = add_box(&mut scene, Vec3::ZERO, Vec3::ONE);
    let mut bytes = NavigationGraph::from_scene(&scene).bake_to_bytes(&scene);
    let starts = u32::from_le_bytes(bytes[44..48].try_into().unwrap()) as usize;
    let at = 48 + starts * 4 + 4;
    bytes[at..at + 4].copy_from_slice(&MARK.to_le_bytes());
    scene.navmesh = Some(bytes.into());
    if stale {
        scene.world.transform_mut(crate_id).unwrap().position.x += 2.0;
    }
    let dir = crate::test_temp::dir().join("rusty_play_navmesh");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name).to_string_lossy().into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    GameWorld::new(
        Rc::new(RefCell::new(loaded)),
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
