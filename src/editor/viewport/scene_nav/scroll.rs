//! src/editor/viewport/scene_nav/scroll.rs — tell a trackpad from a mouse wheel (#1344).
//!
//! egui folds every wheel event into one `smooth_scroll_delta`, but the Scene view
//! gives them different meanings, so this reads the raw events instead. A trackpad
//! reports precise [`egui::MouseWheelUnit::Point`] deltas, a mouse wheel reports
//! lines or pages (winit's `PixelDelta` vs `LineDelta`):
//!
//! | input | meaning |
//! |---|---|
//! | mouse wheel | zoom (fly speed while looking) |
//! | two-finger trackpad scroll | look (yaw/pitch, like RMB look) |
//! | Shift + two-finger trackpad scroll | pan |
//!
//! Wheel events with the zoom modifier (Ctrl/Cmd) are left out: egui already turns
//! them into `zoom_delta`, which the Scene view reads as pinch zoom.

use glam::Vec2;

/// This frame's scroll input, split by device.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    /// Mouse-wheel scroll in points; positive is away from the user.
    pub wheel: f32,
    /// Two-finger trackpad scroll in points, egui's sign (content follows the fingers).
    pub swipe: Vec2,
    /// Shift + two-finger trackpad scroll in points.
    pub swipe_pan: Vec2,
}

/// Split this frame's wheel `events` by device. `line_points` is how many points a
/// wheel line is worth, `page_points` how many a page is.
pub fn split(events: &[egui::Event], line_points: f32, page_points: f32) -> Scroll {
    let mut scroll = Scroll::default();
    for event in events {
        let egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        if modifiers.command || modifiers.ctrl {
            continue;
        }
        let delta = Vec2::new(delta.x, delta.y);
        match unit {
            egui::MouseWheelUnit::Point if modifiers.shift => scroll.swipe_pan += delta,
            egui::MouseWheelUnit::Point => scroll.swipe += delta,
            egui::MouseWheelUnit::Line => scroll.wheel += delta.y * line_points,
            egui::MouseWheelUnit::Page => scroll.wheel += delta.y * page_points,
        }
    }
    scroll
}

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod scroll_tests;
