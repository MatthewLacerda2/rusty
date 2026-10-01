//! Trail and Line components (#441): the `Trail` / `Line` namespaces, the shared
//! style verbs, the scene round-trip, and the trail's fixed-tick recording.

mod line;
mod trail;

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::Scene;

/// Run `f` with the `Trail` and `Line` namespaces registered over `scene`.
fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::trail::register(&lua, scope, scene).unwrap();
        rusty::api::line::register(&lua, scope, scene).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

/// Evaluate `code` and render its results as `Debug` values, comma-joined.
fn eval(lua: &Lua, code: &str) -> String {
    let v: mlua::MultiValue = lua.load(code).eval().unwrap();
    let parts: Vec<String> = v.iter().map(|x| format!("{x:?}")).collect();
    parts.join(", ")
}

/// Save `scene` to a temp file and load it back.
fn round_trip(scene: &Scene, name: &str) -> Scene {
    let path = std::env::temp_dir()
        .join(format!("rusty_{name}_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}
