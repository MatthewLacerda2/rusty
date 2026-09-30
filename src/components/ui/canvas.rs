//! src/components/ui/canvas.rs — Canvas component: the root of an in-game UI (#417).
//!
//! Unity uGUI's `Canvas` + `CanvasScaler` (Scale With Screen Size) folded into one
//! first-class component. An entity carrying a `CanvasComponent` is a UI root: its
//! descendants carrying a [`RectTransformComponent`](crate::components::RectTransformComponent)
//! are laid out inside the canvas rect by the `ui::layout` system. The canvas's own
//! rect is always the whole screen, expressed in *reference units* (see
//! [`CanvasComponent::scale_factor`]); its Transform and any RectTransform it carries
//! are ignored for placement, as Unity drives a root canvas's RectTransform.
//!
//! Pure authoring data — every field serde-persists; the computed rects live in the
//! `ui::UiLayout` resource, never here. The UI model is recorded in `docs/ui.md`.

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Where a canvas draws. Only the screen-space overlay exists today;
/// `ScreenSpaceCamera` and `WorldSpace` arrive with world-space canvases (#429), and
/// adding a variant keeps every saved `ScreenSpaceOverlay` scene loading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasRenderMode {
    /// Drawn on top of the finished frame, in screen space — HUDs and menus.
    #[default]
    ScreenSpaceOverlay,
}

/// Unity's default *Scale With Screen Size* reference resolution.
pub const DEFAULT_REFERENCE_RESOLUTION: Vec2 = Vec2::new(1920.0, 1080.0);

/// A UI root (Unity's `Canvas` + `CanvasScaler`). See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasComponent {
    /// Where the canvas draws (screen-space overlay only, for now).
    pub render_mode: CanvasRenderMode,
    /// Draw and hit-test order across canvases: higher draws on top and is hit first.
    /// Ties keep scene insertion order.
    pub sort_order: i32,
    /// The resolution the UI is authored at. Layout runs in these units; the scale
    /// factor maps them onto the live screen. Each axis is kept ≥ 1.
    pub reference_resolution: Vec2,
    /// 0 = scale to match the screen's width, 1 = its height, in between = a
    /// log-space blend (Unity's *Match Width Or Height*). Kept in `[0, 1]`.
    pub match_width_or_height: f32,
}

impl Default for CanvasComponent {
    fn default() -> Self {
        Self {
            render_mode: CanvasRenderMode::ScreenSpaceOverlay,
            sort_order: 0,
            reference_resolution: DEFAULT_REFERENCE_RESOLUTION,
            match_width_or_height: 0.0,
        }
    }
}

impl CanvasComponent {
    /// Screen pixels per reference unit on a `screen`-pixel screen — Unity's
    /// *Scale With Screen Size* in `Match Width Or Height` mode: the width and height
    /// ratios are blended in log2 space, so a 0.5 match on a screen twice as wide and
    /// equally tall scales by √2 rather than 1.5.
    pub fn scale_factor(&self, screen: Vec2) -> f32 {
        let reference = self.reference_resolution.max(Vec2::ONE);
        let screen = screen.max(Vec2::ONE);
        let log_w = (screen.x / reference.x).log2();
        let log_h = (screen.y / reference.y).log2();
        let t = self.match_width_or_height.clamp(0.0, 1.0);
        (log_w + (log_h - log_w) * t).exp2()
    }

    /// The canvas rect's size in reference units on a `screen`-pixel screen: the
    /// screen divided by the scale factor. Equals the reference resolution only when
    /// the screen's aspect matches it (or the match favours the axis that differs).
    pub fn size(&self, screen: Vec2) -> Vec2 {
        screen.max(Vec2::ONE) / self.scale_factor(screen)
    }
}
