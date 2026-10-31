//! Closing a demo gives the editor back its scene as the same scene (its identity
//! and the file text it last read, so its own next write is no outside change) and
//! no input the replay held (#1598); a drag back on the bar seeks once.

use rusty::app::GameWorld;
use rusty::core::application::DemoRequest;
use rusty::dev::demo::{self, Destination};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

use super::{demo_of_a_saved_scene, editor_world};

fn ask(game: &mut GameWorld, request: DemoRequest) {
    game.resources
        .application
        .borrow_mut()
        .request_demo(request);
}

#[test]
fn closing_puts_back_the_scene_itself_unsaved_edits_and_all() {
    let (path, _) = demo_of_a_saved_scene();
    let (harness, mut ui) = editor_world("identity");
    let mut game = harness.world.borrow_mut();
    let own = crate::temp::dir().join("mine.scene");
    let own = own.to_string_lossy().into_owned();
    game.scene().borrow().save_to_file(&own).unwrap();
    game.scene().borrow_mut().load_from_file(&own).unwrap();
    ui.set_current_scene_path(Some(own.clone()));
    game.scene().borrow_mut().add_entity("unsaved".into());
    let id = game.scene().borrow().id();

    ask(
        &mut game,
        DemoRequest::Open(path.to_string_lossy().into_owned()),
    );
    ui.advance_demo(&mut game, 0.0).unwrap();
    assert_ne!(
        game.scene().borrow().id(),
        id,
        "the demo's scene is another"
    );
    ui.close_demo(&mut game);

    let scene = game.scene().borrow();
    assert_eq!(scene.id(), id, "a bake waiting on it still recognises it");
    let on_disk = std::fs::read_to_string(&own).unwrap();
    assert!(
        scene.is_disk_text(&own, &on_disk),
        "no outside change to reload"
    );
    assert!(scene.has_unsaved_edits(&own), "the edit is still unsaved");
    assert!(scene.find_entity_by_name("unsaved").is_some());
    assert_eq!(ui.current_scene_path().as_deref(), Some(own.as_str()));
}

#[test]
fn a_key_the_recording_held_is_not_held_after_close() {
    let dir = crate::temp::dir();
    let path = dir.join("held.demo");
    let scene_file = dir.join("held.scene").to_string_lossy().into_owned();
    let harness = Harness::new(dir.join("record"), "");
    {
        let mut world = harness.world.borrow_mut();
        world.scene().borrow().save_to_file(&scene_file).unwrap();
        *world.script_manager().scene_path_cell().borrow_mut() = Some(scene_file);
        demo::record(&mut world, Destination::File(path.clone()));
    }
    let code = "Harness.Step(5) Input.Press('W') Harness.Step(5)";
    assert!(scenario::run_code(harness, code, "=record").unwrap().passed);

    let (harness, mut ui) = editor_world("held");
    let mut game = harness.world.borrow_mut();
    ask(
        &mut game,
        DemoRequest::Open(path.to_string_lossy().into_owned()),
    );
    ask(&mut game, DemoRequest::Seek(9));
    ui.advance_demo(&mut game, 0.0).unwrap();
    let console = game.console().borrow().messages.clone();
    assert!(ui.demo_open(), "{console:?}");
    assert!(game.input().borrow().is_key_down("W"), "the replay holds W");
    ui.close_demo(&mut game);
    assert!(
        !game.input().borrow().is_key_down("W"),
        "not into the next Play"
    );
}

#[test]
fn a_drag_back_seeks_once_when_it_ends() {
    let (path, _) = demo_of_a_saved_scene();
    let (harness, mut ui) = editor_world("drag");
    let mut game = harness.world.borrow_mut();
    ask(
        &mut game,
        DemoRequest::Open(path.to_string_lossy().into_owned()),
    );
    ask(&mut game, DemoRequest::Seek(40));
    ui.advance_demo(&mut game, 0.0).unwrap();

    for tick in [30, 20, 10] {
        let view = ui.dev.demo.as_mut().unwrap();
        view.scrub(tick);
        assert_eq!((view.seek, view.shown()), (None, tick), "shown, not sought");
        ui.advance_demo(&mut game, 0.0).unwrap();
        assert_eq!(ui.dev.demo.as_ref().unwrap().current, 40);
    }
    ui.dev.demo.as_mut().unwrap().release();
    ui.advance_demo(&mut game, 0.0).unwrap();
    let view = ui.dev.demo.as_mut().unwrap();
    assert_eq!((view.current, view.target), (10, None));

    view.scrub(50);
    assert_eq!(view.seek, Some(50), "a tick ahead replays at once");
    view.release();
    assert_eq!(view.seek, Some(50), "with nothing pending to seek");
}
