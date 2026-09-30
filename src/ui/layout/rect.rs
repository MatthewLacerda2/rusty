//! src/ui/layout/rect.rs — [`UiRect`]: one laid-out element (#417).

use glam::Vec2;

/// One laid-out UI element (the canvas root included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiRect {
    /// The root canvas entity this element lays out under.
    pub canvas: u32,
    /// Screen pixels per reference unit for that canvas.
    pub scale_factor: f32,
    /// The element's own rect, `(min, size)`, before rotation and scale — in the
    /// same frame as its parent's rect (Unity's layout rect).
    pub rect: (Vec2, Vec2),
    /// The final quad in canvas reference units after the whole rotation / scale
    /// chain: bottom-left, top-left, top-right, bottom-right (Unity's
    /// `GetWorldCorners` order).
    pub corners: [Vec2; 4],
}

impl UiRect {
    /// Axis-aligned bounds of the final quad in reference units, `(min, max)`.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut lo = self.corners[0];
        let mut hi = self.corners[0];
        for c in &self.corners[1..] {
            lo = lo.min(*c);
            hi = hi.max(*c);
        }
        (lo, hi)
    }

    /// Axis-aligned bounds of the final quad in screen pixels, `(min, max)`.
    pub fn screen_bounds(&self) -> (Vec2, Vec2) {
        let (lo, hi) = self.bounds();
        (lo * self.scale_factor, hi * self.scale_factor)
    }
}
