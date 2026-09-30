//! Editor Game-view focus decisions (#416), without a window.

use super::*;

fn interaction(tab: ViewportTab, pressed: bool, hovering: bool) -> ViewportInteraction {
    ViewportInteraction {
        tab,
        origin: egui::pos2(100.0, 40.0),
        size: egui::vec2(400.0, 300.0),
        hover_local: hovering.then(|| egui::vec2(10.0, 10.0)),
        clicked: false,
        dragging: false,
        drag_delta: egui::Vec2::ZERO,
        drag_started: false,
        pointer_pressed: pressed,
    }
}

#[test]
fn a_click_inside_the_game_view_focuses_it() {
    assert!(next_focus(
        false,
        &interaction(ViewportTab::Game, true, true),
        false
    ));
    assert!(!next_focus(
        false,
        &interaction(ViewportTab::Game, false, true),
        false
    ));
}

#[test]
fn a_click_elsewhere_drops_focus_unless_the_cursor_is_captured() {
    let outside = interaction(ViewportTab::Game, true, false);
    assert!(!next_focus(true, &outside, false));
    assert!(
        next_focus(true, &outside, true),
        "a locked cursor can't click away"
    );
}

#[test]
fn the_scene_tab_never_has_game_focus() {
    assert!(!next_focus(
        true,
        &interaction(ViewportTab::Scene, false, true),
        true
    ));
}

#[test]
fn the_rect_is_scaled_to_physical_pixels() {
    let rect = game_view_rect(
        &interaction(ViewportTab::Game, false, false),
        2.0,
        (800, 600),
    );
    assert_eq!(rect.origin, (200.0, 80.0));
    assert_eq!(rect.size, (800.0, 600.0));
    assert_eq!(rect.to_view((600.0, 380.0)), (400.0, 300.0));
}
