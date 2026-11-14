//! Trail and Line components (#441): the `Trail` / `Line` namespaces, the shared
//! style verbs, the scene round-trip, and the trail's fixed-tick recording.

mod line;
mod trail;

use std::cell::RefCell;
use std::rc::Rc;

use crate::lua::eval;
use mlua::Lua;
use rusty::scene::Scene;
use rusty::script::api::bind;

/// Run `f` with the `Trail` and `Line` namespaces registered over `scene`.
fn with_api(scene: &Rc<RefCell<Scene>>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    rusty::script::api::trail::register(&lua, bind(&lua, scene)).unwrap();
    rusty::script::api::line::register(&lua, bind(&lua, scene)).unwrap();
    f(&lua);
}

/// Save `scene` to a temp file and load it back.
fn round_trip(scene: &Scene, name: &str) -> Scene {
    let path = crate::temp::dir()
        .join(format!("rusty_{name}_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}
