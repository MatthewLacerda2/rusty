//! Unit tests for the standalone-player hooks on `GameWorld` (#431): booting straight
//! into Play without an edit snapshot, and `Application.Quit()` raising a request the
//! host consumes exactly once.

use std::cell::RefCell;
use std::rc::Rc;

use crate::app::{GameWorld, PlayTransition};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

fn empty_world() -> GameWorld {
    GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -20.0, 20.0, -20.0, 20.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

#[test]
fn standalone_boot_enters_play_on_the_first_tick_without_a_snapshot() {
    let mut gw = empty_world();
    gw.boot_standalone();
    assert!(gw.is_playing());
    assert!(gw.tick(DT) == PlayTransition::Entered);
    assert!(
        gw.resources.edit_snapshot.is_none(),
        "no edit mode to restore"
    );
    assert!(gw.script_manager().is_live(), "the runtime is up");
}

#[test]
fn editor_play_still_captures_the_edit_snapshot() {
    let mut gw = empty_world();
    gw.set_playing(true);
    gw.tick(DT);
    assert!(gw.resources.edit_snapshot.is_some());
}

#[test]
fn application_quit_raises_a_request_taken_once() {
    let mut gw = empty_world();
    gw.boot_standalone();
    gw.tick(DT);
    assert!(!gw.quit_requested());
    gw.script_manager().eval("Application.Quit()").unwrap();
    assert!(gw.quit_requested());
    assert!(gw.take_quit_request());
    assert!(!gw.quit_requested());
    assert!(!gw.take_quit_request());
}

#[test]
fn application_build_settings_round_trip_through_lua() {
    let mut gw = empty_world();
    gw.boot_standalone();
    gw.tick(DT);
    let eval = |line: &str| gw.script_manager().eval(line).unwrap();
    eval("Application.SetProductName('Neon')");
    eval("Application.SetStartupScene('project/scenes/menu.scene')");
    assert_eq!(eval("Application.GetProductName()"), "Neon");
    assert_eq!(
        eval("Application.GetStartupScene()"),
        "project/scenes/menu.scene"
    );
    assert_eq!(eval("Application.SetWindowMode('Fullscreen')"), "true");
    assert_eq!(eval("Application.SetWindowMode('Borderless')"), "false");
    assert_eq!(eval("Application.GetWindowMode()"), "Fullscreen");
    assert!(gw
        .script_manager()
        .eval("Application.SetStartupScene('  ')")
        .is_err());
}
