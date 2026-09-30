//! Integration coverage for the `Scene` namespace's prefab verbs (issue #350):
//! `SavePrefab` → `Instantiate` round-trips an entity through a `.prefab` file,
//! linked instances record/revert overrides, unpacked copies carry no link, and
//! the error paths (bad root, missing file, bad parent arg) reject cleanly.

mod linked;
mod rejections;

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::scene::Scene;

/// A scene whose root entity sits at (1, 2, 3), plus the tmp `.prefab` path this
/// test round-trips through (unique per test: they run in parallel). The path is
/// interpolated into single-quoted Lua strings, where a Windows `\` would be
/// taken as an escape sequence — so it is normalized to `/`, which `std::fs`
/// accepts on every platform.
fn fixture(tag: &str) -> (RefCell<Scene>, u32, String) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Root".to_string());
    scene.world.transform_mut(root).unwrap().position = Vec3::new(1.0, 2.0, 3.0);
    let dir = std::env::temp_dir();
    let file = format!("rusty_prefab_api_{tag}_{}.prefab", std::process::id());
    let path = dir.join(file).to_string_lossy().replace('\\', "/");
    (RefCell::new(scene), root, path)
}

fn run(scene: &RefCell<Scene>, body: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    let scene_path: RefCell<Option<String>> = RefCell::new(None);
    let is_playing = RefCell::new(false);
    lua.scope(|scope| {
        rusty::api::scene::register(&lua, scope, scene, &scene_path, &is_playing).unwrap();
        body(&lua);
        Ok(())
    })
    .unwrap();
}

fn pos_of(scene: &RefCell<Scene>, id: u32) -> Vec3 {
    scene.borrow().world.transform(id).unwrap().position
}

fn is_linked(scene: &RefCell<Scene>, id: u32) -> bool {
    scene.borrow().world.has_prefab_link(id)
}

fn errors(lua: &Lua, code: &str) -> bool {
    lua.load(code).exec().is_err()
}
