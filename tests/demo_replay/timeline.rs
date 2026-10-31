//! The editor's demo timeline (#1598): a demo opens on the editor's world, plays,
//! scrubs both ways bit for bit, shows its marks, and closing gives the editor its
//! scene and recorder back.

use std::path::PathBuf;

use rusty::core::application::DemoRequest;
use rusty::dev::demo::{self, Destination, Recorder, Replayer};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;
use rusty::editor::EditorUi;

/// A demo of 60 ticks, recorded at 60 Hz, with one mark at tick 20, on a scene saved to disk.
pub fn demo_of_a_saved_scene() -> (PathBuf, String) {
    let dir = crate::temp::dir();
    let scene_file = dir.join("a.scene").to_string_lossy().into_owned();
    let path = dir.join("run.demo");
    let harness = Harness::new(dir.join("record"), "");
    {
        let mut world = harness.world.borrow_mut();
        world.scene().borrow().save_to_file(&scene_file).unwrap();
        *world.script_manager().scene_path_cell().borrow_mut() = Some(scene_file.clone());
        demo::record(&mut world, Destination::File(path.clone()));
    }
    let code = "Harness.Step(20) Debug.Mark('hit') Harness.Step(40)";
    assert!(scenario::run_code(harness, code, "=record").unwrap().passed);
    (path, scene_file)
}

pub fn editor_world(dir: &str) -> (Harness, EditorUi) {
    let harness = Harness::new(crate::temp::dir().join(dir), "");
    let ui = {
        let mut world = harness.world.borrow_mut();
        world.set_playing(false); // the editor starts in edit mode
        world.poll_transition();
        world.resources.application.borrow_mut().mark_demo_viewer();
        EditorUi::new(world.script_manager().scene_path_cell())
    };
    (harness, ui)
}

#[test]
fn a_demo_opens_scrubs_both_ways_and_closes() {
    let (path, scene_file) = demo_of_a_saved_scene();
    let (harness, mut ui) = editor_world("editor");
    let mut game = harness.world.borrow_mut();
    let open = DemoRequest::Open(path.to_string_lossy().into_owned());
    game.resources.application.borrow_mut().request_demo(open);

    ui.advance_demo(&mut game, 0.0).unwrap();
    let view = ui.dev.demo.as_ref().expect("the demo opened");
    assert_eq!((view.ticks, view.current, view.paused), (60, 0, true));
    assert_eq!(view.marks, [(20, "hit".to_string())]);
    assert!(game.scene().borrow().is_playing());
    assert_eq!(
        ui.current_scene_path().as_deref(),
        Some(scene_file.as_str())
    );

    // Forward, then back past where it stood, and forward again: the same world.
    for tick in [40, 10, 59] {
        game.resources
            .application
            .borrow_mut()
            .request_demo(DemoRequest::Seek(tick));
        ui.advance_demo(&mut game, 0.0).unwrap();
        assert_eq!(ui.dev.demo.as_ref().unwrap().current, tick);
    }
    let verdict = game.tap_mut::<Replayer>().unwrap().verdict().clone();
    assert_eq!(verdict.diverged, None, "{verdict:?}");
    assert_eq!(
        game.resources.application.borrow().demo_tick(),
        Some((59, 60))
    );
    assert!(ui.dev.demo.as_ref().unwrap().paused, "the end holds still");
}

#[test]
fn playing_runs_at_the_recorded_speed_and_close_puts_the_editor_back() {
    let (path, _) = demo_of_a_saved_scene();
    let (harness, mut ui) = editor_world("editor");
    let mut game = harness.world.borrow_mut();
    let entities = game.scene().borrow().world.ids().len();
    let open = DemoRequest::Open(path.to_string_lossy().into_owned());
    game.resources.application.borrow_mut().request_demo(open);
    game.resources
        .application
        .borrow_mut()
        .request_demo(DemoRequest::Pause(false));
    ui.advance_demo(&mut game, 0.0).unwrap();
    // Whatever the frame rate: the recording's 60 Hz `dt`, not the display's.
    game.resources
        .application
        .borrow_mut()
        .request_demo(DemoRequest::Seek(0));
    game.resources
        .application
        .borrow_mut()
        .request_demo(DemoRequest::Pause(false));
    ui.advance_demo(&mut game, 0.0).unwrap();
    for _ in 0..10 {
        ui.advance_demo(&mut game, 1.0 / 120.0).unwrap();
    }
    assert_eq!(
        ui.dev.demo.as_ref().unwrap().current,
        5,
        "ten 120 Hz frames"
    );

    // Closing puts the editor back: its scene, no replay, the recorder.
    ui.dev.demo.as_mut().unwrap().close = true;
    ui.advance_demo(&mut game, 0.0).unwrap();
    assert!(ui.dev.demo.is_none() && !game.scene().borrow().is_playing());
    assert_eq!(game.scene().borrow().world.ids().len(), entities);
    assert!(game.tap_mut::<Replayer>().is_none() && game.tap_mut::<Recorder>().is_some());
    assert_eq!(game.resources.application.borrow().demo_tick(), None);
}

#[test]
fn a_bad_demo_leaves_the_editor_as_it_was() {
    let (harness, mut ui) = editor_world("bad");
    let mut game = harness.world.borrow_mut();
    let missing = DemoRequest::Open("no/such.demo".into());
    game.resources
        .application
        .borrow_mut()
        .request_demo(missing);
    ui.advance_demo(&mut game, 0.0);
    assert!(ui.dev.demo.is_none() && !game.scene().borrow().is_playing());
    let errors = game
        .console()
        .borrow()
        .messages
        .iter()
        .any(|(text, _)| text.contains("no/such.demo"));
    assert!(
        errors,
        "the refusal reaches the console: {:?}",
        game.console().borrow().messages
    );
}
