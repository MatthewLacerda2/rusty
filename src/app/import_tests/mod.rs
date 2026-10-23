//! Play and the import cache (#1253): Play refreshes every converted source and the
//! scene's instances first, then reads the cache and never converts, until Stop.
//! Shared fixtures for `gate` (what Play serves) and `refresh` (what it does first).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use glam::Vec3;

use crate::app::GameWorld;
use crate::asset::convert::cache_location;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

mod gate;
mod refresh;

const DT: f32 = 1.0 / 60.0;

/// A quad (6 indices) under the object name `Quad`.
const QUAD: &str = include_str!("../../../tests/fixtures/models/embedded_textures.fbx");

/// One triangle (3 indices) under the same object name.
fn triangle() -> String {
    include_str!("../../../tests/fixtures/models/missing_texture.fbx").replace("Tri", "Quad")
}

fn world() -> GameWorld {
    GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

/// `<test>/assets/enemies/enemy.fbx`, a quad, never imported; returns the root.
fn assets() -> (PathBuf, PathBuf) {
    let root = crate::test_temp::dir().join("assets");
    std::fs::create_dir_all(root.join("enemies")).unwrap();
    let enemy = root.join("enemies/enemy.fbx");
    std::fs::write(&enemy, QUAD).unwrap();
    (root, enemy)
}

fn stamp(path: &Path) -> String {
    let cache = cache_location(path).unwrap();
    std::fs::read_to_string(format!("{}.source", cache.display())).unwrap()
}

fn spawn(gw: &GameWorld, path: &Path) -> Result<u32, String> {
    let mut scene = gw.world.scene.borrow_mut();
    let reference = format!("{}::Quad", path.display());
    instantiate_asset(&mut scene, &reference, None, Vec3::ZERO)
}

fn indices(gw: &GameWorld, id: u32) -> usize {
    let scene = gw.world.scene.borrow();
    let count = scene.world.mesh(id).unwrap().indices.len();
    count
}
