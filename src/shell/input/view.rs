//! src/shell/input/view.rs — window → game-view pointer mapping and wheel units.
//!
//! Scripts read the pointer in **game-view pixels**, origin top-left: the pixel grid
//! the game renders (and lays its UI out) at. In the player that is the whole window;
//! in the editor it is the Game-view panel's rect, whose on-screen size in physical
//! pixels can differ from the render size. Pure math, so it is unit-tested here.

use winit::event::MouseScrollDelta;

/// Where the game view sits in the window, in physical window pixels, and the pixel
/// size the game renders at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameViewRect {
    pub origin: (f64, f64),
    pub size: (f64, f64),
    pub render: (u32, u32),
}

impl GameViewRect {
    /// A view filling a `width × height` window, rendered at that same size.
    pub fn full_window(width: u32, height: u32) -> Self {
        Self {
            origin: (0.0, 0.0),
            size: (f64::from(width), f64::from(height)),
            render: (width, height),
        }
    }

    /// Map a window-pixel pointer position into game-view pixels. Positions outside
    /// the view map outside `0..render` (like Unity's `mousePosition` off-screen);
    /// a degenerate rect maps everything to its origin.
    pub fn to_view(&self, window: (f64, f64)) -> (f64, f64) {
        let axis = |pos: f64, origin: f64, size: f64, render: u32| {
            if size <= 0.0 {
                0.0
            } else {
                (pos - origin) * f64::from(render) / size
            }
        };
        (
            axis(window.0, self.origin.0, self.size.0, self.render.0),
            axis(window.1, self.origin.1, self.size.1, self.render.1),
        )
    }
}

/// Pixels of a touchpad scroll that count as one wheel line.
pub const PIXELS_PER_LINE: f64 = 20.0;

/// The vertical wheel motion in lines (positive = away from the user / scroll up).
/// Touchpads report pixels, which are scaled into lines.
pub fn scroll_lines(delta: MouseScrollDelta) -> f64 {
    match delta {
        MouseScrollDelta::LineDelta(_, y) => f64::from(y),
        MouseScrollDelta::PixelDelta(pos) => pos.y / PIXELS_PER_LINE,
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalPosition;

    use super::*;

    #[test]
    fn full_window_is_identity() {
        let view = GameViewRect::full_window(1280, 720);
        assert_eq!(view.to_view((640.0, 10.0)), (640.0, 10.0));
    }

    #[test]
    fn editor_panel_maps_offset_and_scale() {
        // A panel at (100, 50) shown 400×300 on screen, rendering 800×600.
        let view = GameViewRect {
            origin: (100.0, 50.0),
            size: (400.0, 300.0),
            render: (800, 600),
        };
        assert_eq!(view.to_view((100.0, 50.0)), (0.0, 0.0));
        assert_eq!(view.to_view((300.0, 200.0)), (400.0, 300.0));
        assert_eq!(view.to_view((500.0, 350.0)), (800.0, 600.0));
        assert_eq!(
            view.to_view((90.0, 50.0)),
            (-20.0, 0.0),
            "outside stays outside"
        );
    }

    #[test]
    fn degenerate_rect_does_not_divide_by_zero() {
        let view = GameViewRect::full_window(0, 0);
        assert_eq!(view.to_view((5.0, 5.0)), (0.0, 0.0));
    }

    #[test]
    fn wheel_lines_and_touchpad_pixels() {
        assert_eq!(scroll_lines(MouseScrollDelta::LineDelta(0.0, 1.0)), 1.0);
        let pixels = MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -40.0));
        assert_eq!(scroll_lines(pixels), -2.0);
    }
}
