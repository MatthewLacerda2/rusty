//! Per-tick edge and accumulator semantics of `InputState`.

use super::*;

#[test]
fn key_edges_last_exactly_one_tick() {
    let mut input = InputState::new();
    input.press("w");
    assert!(input.is_key_down("W"), "held state is live");
    assert!(!input.get_key_down("W"), "the edge waits for the next tick");

    input.begin_tick();
    assert!(input.get_key_down("W"));
    assert!(!input.get_key_up("W"));

    input.begin_tick();
    assert!(
        !input.get_key_down("W"),
        "an edge is true for one tick only"
    );
    assert!(input.is_key_down("W"));

    input.release("W");
    input.begin_tick();
    assert!(input.get_key_up("W"));
    assert!(!input.is_key_down("W"));
}

#[test]
fn a_tap_between_ticks_is_not_lost() {
    let mut input = InputState::new();
    input.press("Mouse0");
    input.release("MOUSE0");
    input.begin_tick();
    assert!(input.get_key_down("mouse0"));
    assert!(input.get_key_up("mouse0"));
    assert!(!input.is_key_down("mouse0"));
}

#[test]
fn repeats_and_stray_releases_make_no_edge() {
    let mut input = InputState::new();
    input.release("SPACE");
    input.begin_tick();
    assert!(!input.get_key_up("SPACE"));

    input.press("SPACE");
    input.begin_tick();
    input.press("SPACE"); // OS key-repeat
    input.begin_tick();
    assert!(!input.get_key_down("SPACE"));
}

#[test]
fn deltas_and_text_accumulate_then_clear() {
    let mut input = InputState::new();
    input.add_mouse_delta(3.0, -1.0);
    input.add_mouse_delta(2.0, 4.0);
    input.scroll(1.0);
    input.scroll(0.5);
    input.type_text("h");
    input.type_text("i");
    assert_eq!(
        input.mouse_delta(),
        (0.0, 0.0),
        "published on the next tick"
    );

    input.begin_tick();
    assert_eq!(input.mouse_delta(), (5.0, 3.0));
    assert_eq!(input.scroll_delta(), 1.5);
    assert_eq!(input.text_input(), "hi");

    input.begin_tick();
    assert_eq!(input.mouse_delta(), (0.0, 0.0));
    assert_eq!(input.scroll_delta(), 0.0);
    assert_eq!(input.text_input(), "");
}

#[test]
fn cursor_is_a_recorded_request() {
    let mut input = InputState::new();
    assert_eq!(input.cursor(), CursorState::FREE);
    input.reset_cursor(CursorState::PLAY);
    input.set_cursor_locked(false);
    input.set_cursor_visible(true);
    assert_eq!(input.cursor(), CursorState::FREE);
    input.move_mouse(10.0, 20.0);
    assert_eq!(input.mouse_position(), (10.0, 20.0));
}

#[test]
fn release_all_ends_every_hold_with_an_up_edge() {
    let mut input = InputState::new();
    input.press("W");
    input.press("MOUSE1");
    input.begin_tick();
    input.release_all();
    input.begin_tick();
    assert!(!input.is_key_down("W") && !input.is_key_down("MOUSE1"));
    assert!(input.get_key_up("W") && input.get_key_up("MOUSE1"));
}
