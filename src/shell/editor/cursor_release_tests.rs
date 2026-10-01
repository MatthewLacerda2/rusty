//! Esc frees the cursor in the editor instead of stopping Play (#576), without a window.

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::core::keymap::Keymap;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use crate::shell::input::write_key;

fn click(tab: ViewportTab, inside: bool) -> ViewportInteraction {
    ViewportInteraction {
        tab,
        origin: egui::pos2(0.0, 0.0),
        size: egui::vec2(400.0, 300.0),
        hover_local: inside.then(|| egui::vec2(10.0, 10.0)),
        clicked: false,
        dragging: false,
        drag_delta: egui::Vec2::ZERO,
        drag_started: false,
        pointer_pressed: true,
    }
}

fn released() -> CursorRelease {
    let mut release = CursorRelease::default();
    release.on_key(KeyCode::Escape, true, true);
    release
}

#[test]
fn esc_frees_a_locked_cursor_only_while_the_game_has_input() {
    assert_eq!(
        released().effective(CursorState::PLAY, true),
        CursorState::FREE
    );
    let mut unfocused = CursorRelease::default();
    unfocused.on_key(KeyCode::Escape, true, false);
    assert_eq!(
        unfocused.effective(CursorState::PLAY, true),
        CursorState::PLAY
    );
    let mut key_up = CursorRelease::default();
    key_up.on_key(KeyCode::Escape, false, true);
    assert_eq!(key_up.effective(CursorState::PLAY, true), CursorState::PLAY);
}

#[test]
fn a_click_inside_the_game_view_recaptures_and_one_outside_does_not() {
    let mut outside = released();
    outside.on_pointer(&click(ViewportTab::Game, false));
    assert_eq!(
        outside.effective(CursorState::PLAY, true),
        CursorState::FREE
    );
    let mut scene_tab = released();
    scene_tab.on_pointer(&click(ViewportTab::Scene, true));
    assert_eq!(
        scene_tab.effective(CursorState::PLAY, true),
        CursorState::FREE
    );
    let mut inside = released();
    inside.on_pointer(&click(ViewportTab::Game, true));
    assert_eq!(inside.effective(CursorState::PLAY, true), CursorState::PLAY);
}

#[test]
fn a_play_transition_drops_the_release() {
    let mut release = released();
    release.reset();
    assert_eq!(
        release.effective(CursorState::PLAY, true),
        CursorState::PLAY
    );
}

#[test]
fn esc_reaches_the_game_and_play_keeps_running() {
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.boot_standalone();
    game.set_playing(true);
    game.tick(0.016);
    // The shell's path for a key while the game has input, then the editor's hook.
    write_key(KeyCode::Escape, true, &game, &Keymap::default());
    let mut release = CursorRelease::default();
    release.on_key(KeyCode::Escape, true, true);
    game.tick(0.016);
    assert!(game.is_playing(), "Esc no longer stops Play");
    assert!(game.input().borrow().get_key_down("ESCAPE"));
    let requested = game.input().borrow().cursor();
    assert!(requested.locked, "the game's own request is untouched");
    assert_eq!(release.effective(requested, true), CursorState::FREE);
}
