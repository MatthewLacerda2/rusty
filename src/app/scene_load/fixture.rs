//! Fixture for the `Scene.Load` tests (#432): a menu scene that loads a level on its
//! first Update, with a `DontDestroyOnLoad` music entity, written to a temp dir.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::{Scene, ScriptComponent};
use crate::scripting::ConsoleLogs;

use super::super::GameWorld;

/// Two scene files in a per-test temp dir: `menu.scene` (a `Menu` whose first
/// Update loads the level, and a `Music` that marks itself `DontDestroyOnLoad`) and
/// `level.scene` (one `Level`). Returns `(menu path, level path)`.
pub(super) fn write_scenes(tag: &str) -> (String, String) {
    let dir = crate::test_temp::dir().join(format!("rusty_432_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = |name: &str| dir.join(name).to_string_lossy().replace('\\', "/");
    let level = path("level.scene");
    let script = |name: &str, code: String| -> String {
        let p: PathBuf = dir.join(name);
        std::fs::write(&p, code).unwrap();
        p.to_string_lossy().replace('\\', "/")
    };
    let menu_lua = script(
        "menu.lua",
        format!(
            "local loaded = false\nreturn {{\n\
         Start = function(id) Timer.Invoke(id, function() print('menu:timer') end, 0.05) end,\n\
         Update = function(id) if not loaded then loaded = true; Scene.Load('{level}') end end,\n\
         OnDisable = function(id) print('menu:disable') end,\n\
         OnDestroy = function(id) print('menu:destroy ' .. tostring(Scene.FindEntityByName('Menu') == id)) end,\n}}"
        ),
    );
    let music_lua = script("music.lua", "return {\n\
         Awake = function(id) Scene.DontDestroyOnLoad(id) end,\n\
         Start = function(id) Timer.InvokeRepeating(id, function() print('music:tick') end, 0.05, 0.05) end,\n\
         OnDestroy = function(id) print('music:destroy') end,\n}".to_string());
    let level_lua = script(
        "level.lua",
        "return {\n\
         Awake = function(id) print('level:awake') end,\n\
         Start = function(id) print('level:start ' .. math.random(1, 1000000)) end,\n}"
            .to_string(),
    );

    let mut menu = Scene::new();
    for (name, lua) in [("Menu", menu_lua), ("Music", music_lua)] {
        let id = menu.add_entity(name.to_string());
        *menu.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
            path: lua,
            ..Default::default()
        }];
    }
    let mut lvl = Scene::new();
    let id = lvl.add_entity("Level".to_string());
    *lvl.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: level_lua,
        ..Default::default()
    }];
    menu.save_to_file(&path("menu.scene")).unwrap();
    lvl.save_to_file(&level).unwrap();
    (path("menu.scene"), level)
}

/// A world with `path` loaded as its scene file, and its console.
pub(super) fn world_at(path: &str) -> (GameWorld, Rc<RefCell<ConsoleLogs>>) {
    let mut scene = Scene::new();
    scene.load_from_file(path).unwrap();
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let gw = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -20.0, 20.0, -20.0, 20.0, 1.0,
        ))),
        Rc::clone(&console),
    );
    *gw.script_manager().scene_path_cell().borrow_mut() = Some(path.to_string());
    (gw, console)
}

pub(super) fn logs(console: &Rc<RefCell<ConsoleLogs>>) -> Vec<String> {
    console
        .borrow()
        .messages
        .iter()
        .map(|(m, _)| m.clone())
        .collect()
}
