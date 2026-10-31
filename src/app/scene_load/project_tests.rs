//! Every entry point opens a project the same way (#1652): `GameWorld::open_project`
//! takes `project.rusty`'s settings into `Application`, bound or read-only, and the
//! world runs on its quality presets from the first frame.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::ProjectSource;
use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::core::project::ProjectFile;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// A project file whose startup scene and initial quality preset are not the
/// engine's defaults.
pub const FIXTURE: &str = r#"{
    "build": { "startup_scene": "assets/scenes/arena.scene" },
    "quality": { "default": "High" }
}"#;

fn world() -> GameWorld {
    let nav = NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

fn fixture_file() -> PathBuf {
    let path = crate::test_temp::dir().join("project.rusty");
    std::fs::write(&path, FIXTURE).unwrap();
    path
}

/// The world answers the fixture's settings, through `Application` and the
/// graphics cell the renderer reads.
pub fn assert_runs_the_fixture(game: &GameWorld) {
    let app = game.resources.application.borrow();
    assert_eq!(app.build().startup_scene, "assets/scenes/arena.scene");
    assert_eq!(app.quality(), Some("High"));
    let high = app.quality_presets().get("High").unwrap().values;
    let graphics = game.script_manager().graphics_cell();
    assert_eq!(graphics.borrow().quality, high, "the preset runs at once");
}

fn product_name_on_disk(path: &Path) -> String {
    let file = ProjectFile::read(path).unwrap().unwrap();
    file.build.product_name
}

#[test]
fn a_bound_project_is_read_and_written_back() {
    let path = fixture_file();
    let game = world();
    game.open_project(ProjectSource::Bound(path.clone()));
    assert_runs_the_fixture(&game);
    let mut app = game.resources.application.borrow_mut();
    app.update(|b| b.product_name = "Arena".into()).unwrap();
    assert_eq!(
        product_name_on_disk(&path),
        "Arena",
        "a setter writes the file"
    );
}

#[test]
fn a_read_only_project_is_never_written() {
    let path = fixture_file();
    let settings = ProjectFile::read(&path).unwrap().unwrap().into();
    let game = world();
    game.open_project(ProjectSource::ReadOnly(settings));
    assert_runs_the_fixture(&game);
    let mut app = game.resources.application.borrow_mut();
    app.update(|b| b.product_name = "Arena".into()).unwrap();
    assert_eq!(
        app.build().product_name,
        "Arena",
        "the running value changes"
    );
    assert_ne!(product_name_on_disk(&path), "Arena", "the file does not");
}

#[test]
fn an_unparseable_project_file_is_said_once_and_leaves_the_defaults() {
    crate::logging::init();
    let path = crate::test_temp::dir().join("project.rusty");
    std::fs::write(&path, "{ not json").unwrap();
    let game = world();
    game.open_project(ProjectSource::Bound(path));
    game.apply_project_settings(); // entering Play rereads the file
    assert_eq!(
        game.resources.application.borrow().build(),
        &Default::default()
    );
    // Everything raised: what a session's reply carries, and the console.
    let said = crate::logging::forward(game.console());
    assert_eq!(said.len(), 1, "one message, the parse error: {said:?}");
    assert!(!said[0].contains("last read"), "{said:?}");
    let console = game.console().borrow();
    let levels: Vec<_> = console.messages.iter().map(|(_, l)| l.clone()).collect();
    assert_eq!(levels, [LogLevel::Error], "{:?}", console.messages);
}
