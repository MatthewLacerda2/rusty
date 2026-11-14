//! Unit tests for splitting wheel events by device (#1344).

use super::*;

fn wheel(unit: egui::MouseWheelUnit, x: f32, y: f32, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::MouseWheel {
        unit,
        delta: egui::vec2(x, y),
        phase: egui::TouchPhase::Move,
        modifiers,
    }
}

#[test]
fn trackpad_swipes_and_a_wheel_zooms() {
    let none = egui::Modifiers::NONE;
    let events = [
        wheel(egui::MouseWheelUnit::Point, 3.0, -2.0, none),
        wheel(egui::MouseWheelUnit::Point, 1.0, 0.0, none),
        wheel(egui::MouseWheelUnit::Line, 0.0, 1.0, none),
        wheel(egui::MouseWheelUnit::Page, 0.0, -0.5, none),
        egui::Event::Text("w".into()),
    ];
    let scroll = split(&events, 40.0, 300.0);
    assert_eq!(scroll.swipe, Vec2::new(4.0, -2.0));
    assert_eq!(scroll.swipe_pan, Vec2::ZERO);
    assert_eq!(scroll.wheel, 40.0 - 150.0);
}

#[test]
fn shift_swipe_pans_and_the_zoom_modifier_is_egui_s() {
    let events = [
        wheel(
            egui::MouseWheelUnit::Point,
            5.0,
            0.0,
            egui::Modifiers::SHIFT,
        ),
        wheel(
            egui::MouseWheelUnit::Point,
            9.0,
            9.0,
            egui::Modifiers::COMMAND,
        ),
        wheel(egui::MouseWheelUnit::Line, 0.0, 3.0, egui::Modifiers::CTRL),
    ];
    let scroll = split(&events, 40.0, 300.0);
    assert_eq!(scroll.swipe_pan, Vec2::new(5.0, 0.0));
    assert_eq!(scroll.swipe, Vec2::ZERO);
    assert_eq!(scroll.wheel, 0.0);
}
