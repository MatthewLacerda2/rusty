//! What scripts read from disk (#1062): `require` searches the project, then the
//! engine's scripts, then `package.path`; a read outside the project warns once.

mod dofile;
mod require;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::{Camera, Scene};
use crate::scripting::console::{ConsoleLogs, LogLevel};
use crate::scripting::manager::ScriptManager;
use crate::time::Time;

/// A fresh, empty directory under the temp dir, unique to this test and process.
pub(super) fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rusty_files_{name}_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

pub(super) fn write(path: &Path, code: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, code).expect("write");
}

/// A live VM reading against `workspace`, and its console.
pub(super) fn manager(workspace: &Path) -> (ScriptManager, Rc<RefCell<ConsoleLogs>>) {
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut m = ScriptManager::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::clone(&console),
        Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
        Rc::new(RefCell::new(Time::new())),
    );
    m.init_runtime(&Rc::new(RefCell::new(None))).expect("init");
    m.set_workspace_root(workspace);
    (m, console)
}

pub(super) fn out(m: &ScriptManager, code: &str) -> String {
    m.exec(code).expect("lua runs");
    m.lua
        .as_ref()
        .expect("live")
        .globals()
        .get("out")
        .expect("`out`")
}

pub(super) fn warnings(console: &RefCell<ConsoleLogs>) -> Vec<String> {
    let logs = console.borrow();
    let warned = logs
        .messages
        .iter()
        .filter(|(_, l)| *l == LogLevel::Warning);
    warned.map(|(m, _)| m.clone()).collect()
}
