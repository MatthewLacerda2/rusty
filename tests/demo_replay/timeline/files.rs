//! A demo opened in the editor leaves the operator's files and settings alone
//! (#1598): the replay overwrites the save data, settings, seed and clock with the
//! demo's, on a store bound to no file, and closing gives the editor's back.

use std::path::Path;

use rusty::app::GameWorld;
use rusty::core::application::DemoRequest;
use rusty::core::settings::video::VideoSettings;
use rusty::core::settings::GraphicsSettings;

use super::{demo_of_a_saved_scene, editor_world};

/// Compact, so a flush (which writes pretty JSON) would change its bytes.
const SAVED: &str = r#"{"game":{"coins":7}}"#;
const QUALITY: &str = "Mine";
const SCALE: f32 = 0.5;

/// The editor's own state before the demo: a store bound to `file`, a seed, a
/// time scale, a Game view size and a quality choice of its own.
fn the_operators(game: &mut GameWorld, file: &Path) -> (u64, GraphicsSettings, VideoSettings) {
    std::fs::write(file, SAVED).unwrap();
    game.resources.storage.borrow_mut().open(file).unwrap();
    let seed = game.script_manager().play_seed() ^ 0xdead;
    game.resources.script_manager.set_play_seed(seed);
    game.time().borrow_mut().time_scale = SCALE;
    game.resources.screen.borrow_mut().set_game_view(800, 600);
    let mut app = game.resources.application.borrow_mut();
    app.restore_quality(Some(QUALITY.to_string()));
    drop(app);
    let manager = game.script_manager();
    let settings = (
        *manager.graphics_cell().borrow(),
        *manager.video_cell().borrow(),
    );
    (seed, settings.0, settings.1)
}

fn ask(game: &mut GameWorld, request: DemoRequest) {
    game.resources
        .application
        .borrow_mut()
        .request_demo(request);
}

#[test]
fn opening_scrubbing_and_closing_a_demo_leaves_saved_files_and_settings_alone() {
    let (path, _) = demo_of_a_saved_scene();
    let (harness, mut ui) = editor_world("files");
    let mut game = harness.world.borrow_mut();
    let file = crate::temp::dir().join("storage.json");
    let (seed, graphics, video) = the_operators(&mut game, &file);

    ask(
        &mut game,
        DemoRequest::Open(path.to_string_lossy().into_owned()),
    );
    ui.advance_demo(&mut game, 0.0).unwrap();
    assert!(ui.dev.demo.is_some(), "the demo opened");
    let storage = game.resources.storage.borrow().data();
    assert_eq!(storage, serde_json::json!({}), "the demo's save data");
    // Back past where it stands: a restart, which leaves Play (a Stop flush).
    for tick in [40, 10] {
        ask(&mut game, DemoRequest::Seek(tick));
        ui.advance_demo(&mut game, 0.0).unwrap();
    }
    ui.dev.demo.as_mut().unwrap().close = true;
    ui.advance_demo(&mut game, 0.0).unwrap();
    assert!(ui.dev.demo.is_none());

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        SAVED,
        "saved/ untouched"
    );
    let storage = game.resources.storage.borrow().data();
    assert_eq!(storage, serde_json::json!({"game": {"coins": 7}}));
    assert_eq!(*game.script_manager().graphics_cell().borrow(), graphics);
    assert_eq!(*game.script_manager().video_cell().borrow(), video);
    let quality = game
        .resources
        .application
        .borrow()
        .quality_choice()
        .map(str::to_string);
    assert_eq!(quality.as_deref(), Some(QUALITY));
    assert_eq!(game.script_manager().play_seed(), seed);
    assert_eq!(game.time().borrow().time_scale, SCALE);
    assert_eq!(game.resources.screen.borrow().game_view(), Some((800, 600)));
    // The store is bound to its file again: the editor's own flush still writes.
    game.resources.flush_storage();
    assert_ne!(std::fs::read_to_string(&file).unwrap(), SAVED);
}
