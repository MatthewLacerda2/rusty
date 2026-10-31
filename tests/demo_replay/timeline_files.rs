//! A demo opened in the editor leaves the operator's files and settings alone
//! (#1598): the replay overwrites the save data, settings, seed and clock with the
//! demo's, on a store bound to no file, and closing gives the editor's back.

use rusty::core::application::DemoRequest;

use super::timeline::{demo_of_a_saved_scene, editor_world};

/// Compact, so a flush (which writes pretty JSON) would change its bytes.
const SAVED: &str = r#"{"game":{"coins":7}}"#;

#[test]
fn opening_scrubbing_and_closing_a_demo_leaves_saved_files_and_settings_alone() {
    let (path, _) = demo_of_a_saved_scene();
    let (harness, mut ui) = editor_world("files");
    let mut game = harness.world.borrow_mut();
    let file = crate::temp::dir().join("storage.json");
    std::fs::write(&file, SAVED).unwrap();
    game.resources.storage.borrow_mut().open(&file).unwrap();
    let seed = game.script_manager().play_seed() ^ 0xdead;
    game.resources.script_manager.set_play_seed(seed);
    game.time().borrow_mut().time_scale = 0.5;
    game.resources.screen.borrow_mut().set_game_view(800, 600);
    let quality = Some("Mine".to_string());
    game.resources
        .application
        .borrow_mut()
        .restore_quality(quality.clone());
    let graphics = *game.script_manager().graphics_cell().borrow();
    let video = *game.script_manager().video_cell().borrow();

    let ask = |game: &mut rusty::app::GameWorld, request| {
        game.resources
            .application
            .borrow_mut()
            .request_demo(request);
    };
    ask(
        &mut game,
        DemoRequest::Open(path.to_string_lossy().into_owned()),
    );
    ui.advance_demo(&mut game, 0.0).unwrap();
    assert!(ui.dev.demo.is_some(), "the demo opened");
    let storage = game.resources.storage.borrow().data();
    assert_eq!(
        storage,
        serde_json::json!({}),
        "the replay has the demo's save data"
    );
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
    let app = game.resources.application.borrow();
    assert_eq!(app.quality_choice(), quality.as_deref());
    drop(app);
    assert_eq!(game.script_manager().play_seed(), seed);
    assert_eq!(game.time().borrow().time_scale, 0.5);
    assert_eq!(game.resources.screen.borrow().game_view(), Some((800, 600)));
    // The store is bound to its file again: the editor's own flush still writes.
    game.resources.flush_storage();
    assert_ne!(std::fs::read_to_string(&file).unwrap(), SAVED);
}
