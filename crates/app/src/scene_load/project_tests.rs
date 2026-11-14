//! Every entry point opens a project the same way (#1652): `GameWorld::open_project`
//! takes `project.rusty`'s settings into `Application`, bound or read-only, and the
//! world runs on its quality presets from the first frame.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::ProjectSource;
use crate::scene_load::project_fixture::{assert_runs_the_fixture, FIXTURE};
use crate::GameWorld;
use rusty_core::input::InputState;
use rusty_core::project::ProjectFile;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

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
    let path = rusty_test_support::temp::dir().join("project.rusty");
    std::fs::write(&path, FIXTURE).unwrap();
    path
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
    rusty_test_support::logs::init();
    rusty_test_support::logs::take(); // what earlier code on this thread logged
    let path = rusty_test_support::temp::dir().join("project.rusty");
    std::fs::write(&path, "{ not json").unwrap();
    let game = world();
    game.open_project(ProjectSource::Bound(path));
    game.apply_project_settings(); // entering Play rereads the file
    assert_eq!(
        game.resources.application.borrow().build(),
        &Default::default()
    );
    // Everything raised, as an error: `logging` forwards it to a session's reply
    // and the console as one (its own tests hold that).
    let said = rusty_test_support::logs::take_leveled();
    assert_eq!(said.len(), 1, "one message, the parse error: {said:?}");
    assert_eq!(said[0].0, log::Level::Error, "{said:?}");
    assert!(!said[0].1.contains("last read"), "{said:?}");
}

/// A hand edit of the render-scale default (#1715) reaches the running scale at the
/// next Play, as the presets' do: the value the renderer resolves, not only the
/// one `Graphics.GetDefaultRenderScale` reads.
#[test]
fn a_hand_edited_render_scale_default_runs_at_the_next_play() {
    use rusty_core::settings::render_scale::RenderScaleDefault;
    let path = fixture_file();
    let game = world();
    game.open_project(ProjectSource::Bound(path.clone()));
    let edited = r#"{ "graphics": { "render_scale": "match_display" } }"#;
    std::fs::write(&path, edited).unwrap();
    game.apply_project_settings(); // entering Play rereads the file
    let running = game.script_manager().graphics_cell().borrow().render_scale;
    assert_eq!(running.default, RenderScaleDefault::MatchDisplay);
    let app = game.resources.application.borrow();
    assert_eq!(app.render_scale_default(), RenderScaleDefault::MatchDisplay);
}
