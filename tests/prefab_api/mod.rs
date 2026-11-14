//! Integration coverage for the `Scene` namespace's prefab verbs (issue #350):
//! `SavePrefab` → `Instantiate` round-trips an entity through a `.prefab` file,
//! a linked instance's edits are overrides at once (#1527) and revert, unpacked copies carry no link, and
//! the error paths (bad root, missing file, bad parent arg) reject cleanly.

mod instant;
mod linked;
mod rejections;
mod revert_field;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::scene::Scene;
use rusty::script::api::bind;

/// A scene whose root entity sits at (1, 2, 3), plus the tmp `.prefab` path this
/// test round-trips through (unique per test: they run in parallel), spelled with
/// `/` as a stored path is, since it is interpolated into single-quoted Lua strings.
fn fixture(tag: &str) -> (Rc<RefCell<Scene>>, u32, String) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Root".to_string());
    scene.world.transform_mut(root).unwrap().position = Vec3::new(1.0, 2.0, 3.0);
    let dir = crate::temp::dir();
    let file = format!("rusty_prefab_api_{tag}_{}.prefab", std::process::id());
    let path = dir.join(file).to_string_lossy().into_owned();
    (Rc::new(RefCell::new(scene)), root, path)
}

fn run(scene: &Rc<RefCell<Scene>>, body: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    let scene_path: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    rusty::script::api::scene::register(&lua, bind(&lua, scene), bind(&lua, &scene_path)).unwrap();
    rusty::script::api::transform::register(&lua, bind(&lua, scene)).unwrap();
    body(&lua);
}

fn pos_of(scene: &Rc<RefCell<Scene>>, id: u32) -> Vec3 {
    scene.borrow().world.transform(id).unwrap().position
}

fn is_linked(scene: &Rc<RefCell<Scene>>, id: u32) -> bool {
    scene.borrow().world.has_prefab_link(id)
}

fn errors(lua: &Lua, code: &str) -> bool {
    lua.load(code).exec().is_err()
}
